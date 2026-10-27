"""Launches the actual Tauri WebView and real CLIs. No mocked AI messages."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess

APP = Path(__file__).resolve().parents[1]
ROOT = APP.parent
EXE = APP / 'src-tauri/target/debug/esh-chat-mvp4.exe'
RUN = APP / 'evidence' / ('panels-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))


def hashes():
    return {str(p.relative_to(ROOT/'technical-spike')): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in (ROOT/'technical-spike').rglob('*') if p.is_file()}


def protected():
    h=Path.home()
    paths=[h/'.codex/config.toml',h/'.codex/auth.json',h/'.gemini/antigravity-cli/settings.json',
           h/'.gemini/config/config.json',h/'.gemini/config/mcp_config.json',
           h/'AppData/Roaming/strata/settings.json',h/'.local/share/opencode/auth.json']
    base=h/'.config/opencode'
    if base.exists(): paths.extend(p for p in base.iterdir() if p.is_file())
    return {str(p): hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else None for p in paths}


def write(name,value):
    (RUN/name).write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding='utf8')


def launch(mode,data_dir,extra=None,label=None):
    label=label or mode
    assert data_dir.resolve().is_relative_to(RUN.resolve())
    (data_dir/'gui-probe.json').unlink(missing_ok=True)
    env=os.environ.copy()
    for key in ['ESH_CODEX_BIN','ESH_AGY_BIN','ESH_ARCH_TIMEOUT_SECONDS']:
        env.pop(key,None)
    env.update(ESH_DATA_DIR=str(data_dir),ESH_GUI_PROBE=mode)
    # Each data directory gets its own WebView2 profile. WebView2 creates its
    # EBWebView folder inside the given folder, so pointing it at the data directory
    # reproduces the app's default layout (EBWebView next to chat.sqlite). The
    # timeout kill below can then only damage a throwaway profile, never the
    # user's, and localStorage UI state follows the DB across a restore launch.
    data_dir.mkdir(parents=True,exist_ok=True)
    env['WEBVIEW2_USER_DATA_FOLDER']=str(data_dir)
    if extra: env.update(extra)
    with (RUN/(label+'-stdout.log')).open('w',encoding='utf8') as out, (RUN/(label+'-stderr.log')).open('w',encoding='utf8') as err:
        p=subprocess.Popen([str(EXE)],cwd=APP,env=env,stdout=out,stderr=err)
        try: code=p.wait(timeout=700 if mode=='workspace' else 400)
        except subprocess.TimeoutExpired:
            subprocess.run(['taskkill','/PID',str(p.pid),'/T','/F'],capture_output=True)
            raise RuntimeError(f'{mode}: native GUI timeout')
    probe_path=data_dir/'gui-probe.json'
    if not probe_path.exists(): raise RuntimeError(f'{mode}: no WebView evidence (exit={code})')
    probe=json.loads(probe_path.read_text(encoding='utf8'))
    write(label+'-gui.json',probe)
    assert code==0,(mode,code)
    assert 'failed' not in probe,probe
    messages=[m for m in probe['snapshot']['messages'] if m['chatSessionId']==probe['selectedSessionId']]
    expected=[m for m in messages if m['channel']=='main' and m['speaker'] in ['user','codex']]
    expected+=sorted([m for m in messages if m['channel']=='collaboration' and m['kind'] in ['review_request','review_followup','review_reply'] and m['speaker']!='user'],key=lambda m:(m['createdAt'],m['sequence']))
    assert [(m['id'],m['speaker'],m['content'],m['sequence']) for m in probe['rendered']]==[
        (m['id'],m['speaker'],m['content'],m['sequence']) for m in expected],f'{mode}: routed DOM != SQLite snapshot'
    assert all(m['renderedText'].strip() for m in probe['rendered']), 'Empty rendered message'
    assert all(m['channel']==m['panel'] for m in probe['rendered'])
    assert all(m['speaker'] in ['user','codex'] for m in probe['rendered'] if m['panel']=='main')
    assert all(m['speaker'] in ['codex','antigravity'] for m in probe['rendered'] if m['panel']=='collaboration')
    assert probe['collaborationEditableCount']==0
    assert set(probe['archivedIds'])=={m['id'] for m in messages if m['channel'] in ['activity','legacy']}
    assert all(r['after']-r['before']>=20 for r in probe['resizeResults']),probe['resizeResults']
    if probe['navigation']:
        nav=probe['navigation'];assert nav['writable']
        nav_messages=[m for m in probe['snapshot']['messages'] if m['chatSessionId']==nav['sessionId'] and m['channel'] in ['main','collaboration']]
        assert set(nav['messageIds'])=={m['id'] for m in nav_messages}
    return probe


def main():
    RUN.mkdir(parents=True,exist_ok=False)
    before=protected();write('protected-before.json',before)
    write('source-before.json',hashes())
    result={}
    try:
        data=RUN/'live-data'
        live=launch('live',data)
        assert {'user','codex','antigravity'} <= {m['speaker'] for m in live['rendered']}
        assert not any(m['isError'] for m in live['snapshot']['messages'])
        db=sqlite3.connect(data/'chat.sqlite')
        rows=[{'sequence':r[0],'createdAt':r[1],'kind':r[2],'payload':json.loads(r[3])} for r in db.execute('SELECT sequence,created_at,kind,payload FROM audit ORDER BY sequence')]
        write('live-audit.json',rows)
        calls=[r for r in rows if r['kind']=='codex_rx' and r['payload'].get('method')=='item/tool/call']
        assert len(calls)==2,'Expected first review and real follow-up review'
        params=calls[0]['payload']['params']
        for index,record in enumerate(calls):
            call=record['payload'];p=call['params']
            assert p['threadId']==params['threadId'] and p['turnId']==params['turnId']
            tx=[r for r in rows if r['kind']=='codex_tx' and r['payload'].get('id')==call['id'] and 'result' in r['payload']]
            assert len(tx)==1,'Tool response must have exactly one wire write'
            completed=[r for r in rows if r['kind']=='codex_rx' and r['payload'].get('method')=='item/completed'
                       and r['payload'].get('params',{}).get('item',{}).get('id')==p['callId']]
            assert len(completed)==1 and completed[0]['payload']['params']['item']['success'] is True
            assert not [r for r in rows if record['sequence']<r['sequence']<tx[0]['sequence'] and r['kind']=='codex_rx' and r['payload'].get('method')=='turn/completed']
            request=next(m for m in live['rendered'] if m['collaborationId']==p['callId'] and m['speaker']=='codex')
            reply=next(m for m in live['rendered'] if m['collaborationId']==p['callId'] and m['speaker']=='antigravity')
            assert request['panel']==reply['panel']=='collaboration'
            assert request['kind']==('review_request' if index==0 else 'review_followup')
            assert request['content']==p['arguments']['question']
            envelope=json.loads(tx[0]['payload']['result']['contentItems'][0]['text'])
            assert reply['content']==envelope['archReview']['answer']
        turns=[r for r in rows if r['kind']=='codex_turn_completed']
        assert len(turns)==1 and turns[0]['payload']['turnId']==params['turnId']
        assert turns[0]['payload']['threadId']==params['threadId']
        arch_results=[r['payload']['result'] for r in rows if r['kind']=='arch_rx' and r['payload'].get('event')=='result']
        assert len(arch_results)==2 and all(r['status']=='SUCCESS' for r in arch_results)
        for arch_result in arch_results:
            assert any(m['speaker']=='antigravity' and m['content']==arch_result['structured_output']['answer'] and m['panel']=='collaboration' for m in live['rendered'])
        final=[r['payload']['params']['item']['text'] for r in rows if r['kind']=='codex_rx'
               and r['payload'].get('method')=='item/completed'
               and r['payload'].get('params',{}).get('item',{}).get('type')=='agentMessage'
               and r['sequence']>tx[0]['sequence']]
        assert any('SAME_TURN_RESUMED' in t and any(m['speaker']=='codex' and m['content']==t and m['panel']=='main' for m in live['rendered']) for t in final)
        delivery=db.execute('SELECT state FROM tool_deliveries').fetchall()
        assert delivery==[('confirmed',),('confirmed',)],delivery
        saved=[(m['id'],m['content'],m['sequence']) for m in live['rendered']]
        db.close()
        restored=launch('restore',data)
        assert [(m['id'],m['content'],m['sequence']) for m in restored['rendered']]==saved
        assert [(a['id'],a['status'],a['detail']) for a in restored['snapshot']['agents']]==[(a['id'],a['status'],a['detail']) for a in live['snapshot']['agents']]
        error=launch('error',RUN/'error-data',{'ESH_AGY_BIN':str(RUN/'missing-agy.exe')})
        assert any('ESH_AGY_BIN' in m for m in error['alerts'])
        assert not any(m['speaker']=='antigravity' for m in error['rendered'])
        # Read an old pre-channel DB into a COPY; leave prior evidence untouched.
        legacy=APP/'evidence/gui-20261018-032922/live-data/chat.sqlite'
        old=sqlite3.connect(f'file:{legacy.as_posix()}?mode=ro',uri=True)
        original_records=old.execute('SELECT sequence,id,speaker,content,created_at FROM messages ORDER BY sequence').fetchall()
        migrated_dir=RUN/'migration-data';migrated_dir.mkdir()
        copied=sqlite3.connect(migrated_dir/'chat.sqlite');old.backup(copied);old.close();copied.close()
        migration=launch('restore-history',migrated_dir,label='migration')
        after=sqlite3.connect(migrated_dir/'chat.sqlite')
        for record in original_records:
            assert after.execute('SELECT sequence,id,speaker,content,created_at FROM messages WHERE id=?',(record[1],)).fetchone()==record
        after.close()
        assert any(m['kind']=='review_request' for m in migration['rendered'])
        assert any(m['kind']=='review_reply' for m in migration['rendered'])
        result={'passed':True,'nativeGui':True,'mockAi':False,'messagesRendered':len(saved),
                'mainMessages':len([m for m in live['rendered'] if m['panel']=='main']),
                'collaborationMessages':len([m for m in live['rendered'] if m['panel']=='collaboration']),
                'sameThreadId':params['threadId'],'sameTurnId':params['turnId'],
                'callIds':[c['payload']['params']['callId'] for c in calls],
                'toolWireResponsesPerCall':1,'toolCompletionEventsPerCall':1,
                'firstAndFollowupReviewVisible':True,'collaborationReadOnly':True,'bothPanelDragsVerified':True,'sessionNavigationVerified':True,
                'legacyRecordsPreserved':len(original_records),'legacyProtocolBackfillVerified':True,
                'toolDeliveryState':'confirmed','restartRestoredMessages':len(saved),'restartRestoredAgentStates':True,'connectionErrorVisible':True}
    except Exception as e:
        result={'passed':False,'error':repr(e)}
        raise
    finally:
        original=json.loads((APP/'evidence/technical-spike-before.json').read_text(encoding='utf8'))
        result['technicalSpikeUnchanged']=hashes()==original
        after=protected();write('protected-after.json',after)
        result['protectedFilesUnchanged']=before==after
        write('result.json',result)
        print(json.dumps({'evidence':str(RUN),**result},ensure_ascii=False,indent=2),flush=True)


if __name__=='__main__':main()
