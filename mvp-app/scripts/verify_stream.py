"""Actual Codex deltas observed in the native WebView, cancellation and Arch recovery."""
import datetime
import json
import sqlite3
import verify_live as verify


def main():
    verify.RUN=verify.APP/'evidence'/('stream-'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True,exist_ok=False)
    protected,spike=verify.protected(),verify.hashes()
    result={'passed':False,'mockAi':False}
    try:
        data=verify.RUN/'data'
        probe=verify.launch('stream',data)
        stream=probe['streaming']; messages=probe['snapshot']['messages']
        assert stream['updatesBeforeCompletion']>=2 and stream['singleFinalRow']
        final=next(m for m in messages if m['id']==stream['finalId'])
        partial=next(m for m in messages if m['id']==stream['partialId'])
        assert final['delivery']=='delivered' and partial['delivery']=='interrupted' and partial['content']
        db=sqlite3.connect(data/'chat.sqlite')
        rows=[{'kind':k,'payload':json.loads(p)} for k,p in db.execute('SELECT kind,payload FROM audit ORDER BY sequence')]
        verify.write('audit.json',rows)
        completed=[r['payload']['params'] for r in rows if r['kind']=='codex_rx' and r['payload'].get('method')=='item/completed' and r['payload'].get('params',{}).get('item',{}).get('type')=='agentMessage']
        item=next(p for p in completed if p['item']['text']==final['content'])
        deltas=[r['payload']['params']['delta'] for r in rows if r['kind']=='codex_rx' and r['payload'].get('method')=='item/agentMessage/delta' and r['payload'].get('params',{}).get('itemId')==item['item']['id']]
        assert len(deltas)>=2 and ''.join(deltas)==final['content']
        assert len([m for m in messages if m['speaker']=='codex' and m['turnId']==final['turnId'] and m['channel']=='main'])==1
        assert any(r['kind']=='codex_cancel_terminal' and r['payload']['status']=='cancelled' for r in rows)
        assert any(r['kind']=='arch_rx' and r['payload'].get('event')=='result' and r['payload']['result']['status']=='SUCCESS' for r in rows)
        assert db.execute('SELECT state FROM tool_deliveries').fetchall()==[('confirmed',)]
        call=next(r['payload'] for r in rows if r['kind']=='codex_rx' and r['payload'].get('method')=='item/tool/call')
        replies=[r for r in rows if r['kind']=='codex_tx' and r['payload'].get('id')==call['id'] and 'result' in r['payload']]
        assert len(replies)==1
        recovered=next(m for m in messages if m['speaker']=='codex' and m['channel']=='main' and 'STREAM_RECOVERED' in m['content'])
        assert recovered['turnId']==call['params']['turnId'] and recovered['threadId']==call['params']['threadId']
        assert any(m['speaker']=='antigravity' and m['panel']=='collaboration' for m in probe['rendered'])
        assert len([r for r in rows if r['kind']=='codex_session'])==1
        db.close()
        restored=verify.launch('restore',data,label='restored')
        assert restored['snapshot']['messages']==messages
        assert not any(m['delivery']=='streaming' for m in restored['snapshot']['messages'])
        result.update(passed=True,visibleUpdatesBeforeCompletion=stream['updatesBeforeCompletion'],realDeltaCount=len(deltas),
                      sourceMatchesCliFinal=True,noDuplicateFinal=True,cancelledPartialRestored=True,
                      sameConnectionRecovery=True,archReviewAndRecovery=True,restartRestored=True)
    finally:
        result['protectedUnchanged']=protected==verify.protected()
        result['technicalSpikeUnchanged']=spike==verify.hashes()
        verify.write('result.json',result)
        print(verify.RUN)
        print(result)
    assert result['passed'] and result['protectedUnchanged'] and result['technicalSpikeUnchanged']


if __name__=='__main__':main()
