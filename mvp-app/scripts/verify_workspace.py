"""Real GUI creation, per-session CLI resume, and existing Codex import tests."""
import datetime
import json
import sqlite3
import verify_live as verify


def main():
    verify.RUN=verify.APP/'evidence'/('workspace-'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True,exist_ok=False)
    before=verify.protected();verify.write('protected-before.json',before)
    result={'passed':False}
    try:
        data=verify.RUN/'data'
        live=verify.launch('workspace',data)
        w=live['workspace'];s=live['snapshot'];sessions={v['id']:v for v in s['sessions']}
        assert any(r['id']==w['roomId'] and r['name']=='검증 프로젝트' for r in s['rooms'])
        assert all(sessions[w[k]]['roomId']==w['roomId'] for k in ['sourceLocalId','isolatedLocalId','copiedLocalId','resumedLocalId'])
        assert sessions[w['sourceLocalId']]['threadId']==w['sourceThread']
        assert sessions[w['resumedLocalId']]['threadId']==w['sourceThread']
        assert sessions[w['copiedLocalId']]['threadId']!=w['sourceThread']
        assert sessions[w['copiedLocalId']]['mode']=='copy' and sessions[w['resumedLocalId']]['mode']=='resume'
        for key,tag in [('sourceLocalId','RESUMED_A'),('copiedLocalId','COPY_READY'),('resumedLocalId','ORIGINAL_RESUME_READY')]:
            assert any(m['chatSessionId']==w[key] and m['speaker']=='codex' and m['channel']=='main' and tag in m['content'] and 'WORKSPACE_SOURCE_8417' in m['content'] for m in s['messages'])
        db=sqlite3.connect(data/'chat.sqlite')
        audit=[{'sequence':r[0],'kind':r[1],'payload':json.loads(r[2])} for r in db.execute('SELECT sequence,kind,payload FROM audit ORDER BY sequence')]
        verify.write('audit.json',audit)
        sent=[r['payload'] for r in audit if r['kind']=='codex_tx' and r['payload'].get('method')=='turn/start']
        b_thread=sessions[w['isolatedLocalId']]['threadId']
        assert any(t['params']['threadId']==b_thread for t in sent)
        for t in sent:
            text=t['params']['input'][0]['text']
            if t['params']['threadId']==b_thread:assert 'WORKSPACE_SOURCE_8417' not in text
            else:assert 'SESSION_B_READY' not in text
        resumes=[r['payload']['params']['threadId'] for r in audit if r['kind']=='codex_tx' and r['payload'].get('method')=='thread/resume']
        assert resumes.count(w['sourceThread'])>=2
        assert any(r['kind']=='codex_tx' and r['payload'].get('method')=='thread/list' for r in audit)
        assert any(r['kind']=='codex_tx' and r['payload'].get('method') in ['thread/read','thread/turns/list'] for r in audit)
        calls=[r['payload'] for r in audit if r['kind']=='codex_rx' and r['payload'].get('method')=='item/tool/call']
        assert len(calls)==1 and calls[0]['params']['threadId']==sessions[w['copiedLocalId']]['threadId']
        call=calls[0]
        answers=[r for r in audit if r['kind']=='codex_tx' and r['payload'].get('id')==call['id'] and 'result' in r['payload']]
        assert len(answers)==1 and answers[0]['payload']['result']['success']
        assert db.execute("SELECT state FROM tool_deliveries").fetchall()==[('confirmed',)]
        db.close()
        restored=verify.launch('restore',data,label='restored')
        assert restored['snapshot']['rooms']==s['rooms'] and restored['snapshot']['sessions']==s['sessions']
        assert restored['snapshot']['messages']==s['messages']
        assert restored['selectedSessionId']==w['resumedLocalId']
        result={'passed':True,'nativeGui':True,'mockAi':False,'roomCreated':True,'sessionsCreated':True,'sessionContextsIsolated':True,
                'existingCodexListAndHistory':True,'historyCopyWithArch':True,'originalSessionResumed':True,
                'switchBackResumesSameThread':True,'roomsSessionsMessagesRestored':True,'sourceThreadId':w['sourceThread'],
                'copyThreadId':sessions[w['copiedLocalId']]['threadId'],'toolResponseCount':len(answers)}
    finally:
        after=verify.protected();verify.write('protected-after.json',after)
        result['protectedFilesUnchanged']=before==after
        original=json.loads((verify.APP/'evidence/technical-spike-before.json').read_text(encoding='utf8'))
        result['technicalSpikeUnchanged']=verify.hashes()==original
        verify.write('result.json',result)
        print(json.dumps({'evidence':str(verify.RUN),**result},ensure_ascii=False,indent=2),flush=True)


if __name__=='__main__':main()
