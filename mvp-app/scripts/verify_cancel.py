"""Real Codex/Arch cancellation via the native GUI; never supplies AI replies."""
import ctypes
import datetime
import json
import sqlite3
import verify_live as verify


def exited(pid):
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.GetExitCodeProcess.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    handle = kernel.OpenProcess(0x1000,False,pid)
    if not handle:
        assert ctypes.get_last_error()==87, 'Cannot confirm Arch process exit'
        return True
    try:
        code = ctypes.c_ulong()
        assert kernel.GetExitCodeProcess(handle,ctypes.byref(code))
        return code.value != 259
    finally:
        kernel.CloseHandle(handle)


def main():
    verify.RUN=verify.APP/'evidence'/('cancel-'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True,exist_ok=False)
    protected,spike=verify.protected(),verify.hashes()
    result={'passed':False,'mockAi':False}
    try:
        data=verify.RUN/'data'
        probe=verify.launch('cancel',data)
        c=probe['cancellation']
        db=sqlite3.connect(data/'chat.sqlite')
        audit=[{'sequence':r[0],'kind':r[1],'payload':json.loads(r[2])} for r in db.execute('SELECT sequence,kind,payload FROM audit ORDER BY sequence')]
        verify.write('audit.json',audit)
        terminal=[a for a in audit if a['kind']=='codex_cancel_terminal']
        assert len(terminal)==2 and all(a['payload']['status']=='cancelled' for a in terminal)
        finished=[a for a in audit if a['kind']=='cancel_finished']
        assert len(finished)==2 and {a['payload']['runId'] for a in finished}=={c['normalRun'],c['archRun']}
        assert all(a['payload']['status']=='cancelled' for a in finished)
        sessions=[a for a in audit if a['kind']=='codex_session']
        assert len(sessions)==1 and sessions[0]['payload']['threadId']==c['thread']
        turns=[a['payload']['params'] for a in audit if a['kind']=='codex_tx' and a['payload'].get('method')=='turn/start']
        assert len(turns)==4 and all(t['threadId']==c['thread'] for t in turns)
        assert all('CANCELLED_QUEUED_INPUT' not in t['input'][0]['text'] for t in turns)
        calls=[a['payload'] for a in audit if a['kind']=='codex_rx' and a['payload'].get('method')=='item/tool/call']
        assert len(calls)==1
        answers=[a for a in audit if a['kind']=='codex_tx' and a['payload'].get('id')==calls[0]['id'] and 'result' in a['payload']]
        assert len(answers)==1 and not answers[0]['payload']['result']['success']
        envelope=json.loads(answers[0]['payload']['result']['contentItems'][0]['text'])
        assert envelope['archReview']['status']=='cancelled'
        arch=[a['payload'] for a in audit if a['kind']=='arch_started']
        assert len(arch)==1 and exited(arch[0]['pid'])
        assert any(a['kind']=='arch_exit' and a['payload']['cancelled'] and not a['payload']['success'] for a in audit)
        assert not any(m['speaker']=='antigravity' for m in probe['snapshot']['messages'])
        assert db.execute("SELECT delivery FROM messages WHERE content LIKE 'CANCELLED_QUEUED_INPUT%'").fetchone()==('cancelled',)
        db.close()
        restored=verify.launch('restore',data,label='restored')
        assert restored['snapshot']['messages']==probe['snapshot']['messages']
        assert restored['snapshot']['activeRun'] is None
        result.update(passed=True,normalTurnInterrupted=True,archProcessExited=True,openToolCancelledOnce=True,
                      sameConnectionAndThreadRecovery=True,queuedInputCancelled=True,staleRunRejected=c['staleRejected'],restartRestored=True)
    finally:
        result['protectedUnchanged']=protected==verify.protected()
        result['technicalSpikeUnchanged']=spike==verify.hashes()
        verify.write('result.json',result)
        print(verify.RUN)
        print(result)
    assert result['passed'] and result['protectedUnchanged'] and result['technicalSpikeUnchanged']


if __name__=='__main__':main()
