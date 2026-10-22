"""Real native composer selection, Codex model switch, Arch review and restart."""
import datetime
import json
import sqlite3
import subprocess
import verify_live as verify


def main():
    verify.RUN = verify.APP / 'evidence' / ('models-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True, exist_ok=False)
    protected, spike = verify.protected(), verify.hashes()
    result = {'passed': False}
    try:
        home = verify.Path.home()
        codex = max((home/'AppData/Local/OpenAI/Codex/bin').glob('*/codex.exe'), key=lambda p:p.stat().st_mtime)
        versions = {}
        for name, exe in [('codex',codex),('antigravity',home/'.gemini/bin/agy.exe')]:
            p = subprocess.run([str(exe),'--version'], capture_output=True, text=True, encoding='utf8', timeout=30, creationflags=subprocess.CREATE_NO_WINDOW)
            assert p.returncode == 0, p.stderr
            versions[name] = p.stdout.strip()
        verify.write('versions.json', versions)
        data = verify.RUN / 'data'
        probe = verify.launch('models', data)
        m = probe['modelSelection']
        assert m['sameThread'] and m['optionsMatch'] and m['invalidRejected']
        assert m['firstModel'] != m['secondModel']
        db = sqlite3.connect(data/'chat.sqlite')
        assert db.execute('PRAGMA user_version').fetchone()[0] == 4
        audit = [{'sequence':r[0],'kind':r[1],'payload':json.loads(r[2])} for r in db.execute('SELECT sequence,kind,payload FROM audit ORDER BY sequence')]
        verify.write('audit.json',audit)
        tx = [a['payload']['params'] for a in audit if a['kind']=='codex_tx' and a['payload'].get('method')=='turn/start']
        assert len(tx)==2
        for turn, model, effort in zip(tx,[m['firstModel'],m['secondModel']],[m['firstEffort'],m['secondEffort']]):
            assert turn['threadId']==m['thread'] and turn['model']==model and turn['effort']==effort
        completed = [a for a in audit if a['kind']=='codex_turn_completed']
        assert len(completed)==2 and all(a['payload']['threadId']==m['thread'] for a in completed)
        calls = [a['payload'] for a in audit if a['kind']=='codex_rx' and a['payload'].get('method')=='item/tool/call']
        assert len(calls)==1 and calls[0]['params']['threadId']==m['thread']
        replies = [a for a in audit if a['kind']=='codex_tx' and a['payload'].get('id')==calls[0]['id'] and 'result' in a['payload']]
        assert len(replies)==1 and replies[0]['payload']['result']['success']
        assert calls[0]['params']['turnId']==completed[1]['payload']['turnId']
        assert db.execute('SELECT state FROM tool_deliveries').fetchall()==[('confirmed',)]
        assert any(a['kind']=='arch_rx' and a['payload'].get('event')=='result' and a['payload']['result']['status']=='SUCCESS' for a in audit)
        assert db.execute('SELECT model,reasoning_effort FROM chat_sessions WHERE id=?',(m['sessionA'],)).fetchone()==(m['secondModel'],m['secondEffort'])
        assert db.execute('SELECT model,reasoning_effort FROM chat_sessions WHERE id=?',(m['sessionB'],)).fetchone()==(m['firstModel'],m['firstEffort'])
        db.close()
        restored = verify.launch('restore',data,label='restored')
        assert restored['snapshot']['messages']==probe['snapshot']['messages']
        assert restored['snapshot']['sessions']==probe['snapshot']['sessions']
        assert restored['composerSelection']=={'model':m['secondModel'],'effort':m['secondEffort']}
        error = verify.launch('models-error',verify.RUN/'error-data',{'ESH_CODEX_BIN':str(verify.RUN/'missing-codex.exe')})
        assert error['modelSelection']['catalogErrorVisible'] and error['modelSelection']['pickerDisabled']
        result = {'passed':True,'mockAi':False,'versions':versions,'realCatalog':True,'modelSwitchSameThread':True,
                  'firstModel':m['firstModel'],'firstEffort':m['firstEffort'],'secondModel':m['secondModel'],'secondEffort':m['secondEffort'],
                  'archReviewSameTurnResumed':True,'toolResponses':len(replies),'perSessionSettingsRestored':True,
                  'invalidSelectionRejected':True,'catalogErrorVisible':True}
    finally:
        result['protectedUnchanged'] = protected == verify.protected()
        result['technicalSpikeUnchanged'] = spike == verify.hashes()
        verify.write('result.json',result)
        print(verify.RUN)
        print(json.dumps(result,ensure_ascii=False))
    assert result['passed'] and result['protectedUnchanged'] and result['technicalSpikeUnchanged']


if __name__=='__main__':
    main()
