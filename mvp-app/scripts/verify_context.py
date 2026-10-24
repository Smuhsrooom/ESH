"""Real usage and automatic compaction with a process-only low threshold in debug probe mode."""
import datetime
import json
import sqlite3
import verify_live as verify


def main():
    verify.RUN=verify.APP/'evidence'/('context-'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True,exist_ok=False)
    protected,spike=verify.protected(),verify.hashes()
    result={'passed':False,'mockAi':False,'testOnlyAutoCompactLimit':3000}
    try:
        data=verify.RUN/'data'
        probe=verify.launch('context',data)
        state=probe['contextUsage']['context']
        db=sqlite3.connect(data/'chat.sqlite')
        rows=[{'sequence':n,'kind':k,'payload':json.loads(p)} for n,k,p in db.execute('SELECT sequence,kind,payload FROM audit ORDER BY sequence')]
        verify.write('audit.json',rows)
        assert db.execute('PRAGMA user_version').fetchone()[0]==5
        db.close()
        usage=[r['payload']['params']['tokenUsage'] for r in rows if r['kind']=='codex_rx' and r['payload'].get('method')=='thread/tokenUsage/updated' and r['payload']['params']['threadId']==state['threadId']]
        assert usage and usage[-1]['modelContextWindow']==state['usage']['modelContextWindow']
        for group in ('last','total'):
            assert all(usage[-1][group][k]==v for k,v in state['usage'][group].items())
        compactions=[r for r in rows if r['kind']=='codex_rx' and r['payload'].get('method')=='item/completed' and r['payload'].get('params',{}).get('item',{}).get('type')=='contextCompaction']
        assert state['compaction']=='completed' and len({r['payload']['params']['item']['id'] for r in compactions})==state['compactionCount']>=1
        assert any(r['kind']=='codex_rx' and r['payload'].get('method')=='item/started' and r['payload']['params']['item']['type']=='contextCompaction' for r in rows)
        assert any(r['kind']=='codex_spawn' and 'model_auto_compact_token_limit=3000' in r['payload']['args'] for r in rows)
        assert any(r['kind']=='arch_rx' and r['payload'].get('event')=='result' and r['payload']['result']['status']=='SUCCESS' for r in rows)
        call=next(r['payload'] for r in rows if r['kind']=='codex_rx' and r['payload'].get('method')=='item/tool/call')
        tx=[r for r in rows if r['kind']=='codex_tx' and r['payload'].get('id')==call['id'] and 'result' in r['payload']]
        assert len(tx)==1
        final=next(m for m in probe['snapshot']['messages'] if m['speaker']=='codex' and m['channel']=='main' and 'CONTEXT_RECOVERED_7319' in m['content'])
        assert final['turnId']==call['params']['turnId'] and final['threadId']==call['params']['threadId']
        restored=verify.launch('restore',data,label='restored')
        assert restored['snapshot']['contexts']==probe['snapshot']['contexts']
        assert restored['snapshot']['messages']==probe['snapshot']['messages']
        assert '압축 완료' in restored['contextDisplay']
        result.update(passed=True,actualUsageEvents=len(usage),actualCompactions=len(compactions),
                      guiCompactingAndCompleted=True,sessionIsolation=True,restartRestored=True,
                      archSameTurnResumed=True,toolResponseWireWrites=len(tx))
    finally:
        result['protectedUnchanged']=protected==verify.protected()
        result['technicalSpikeUnchanged']=spike==verify.hashes()
        verify.write('result.json',result)
        print(verify.RUN);print(result)
    assert result['passed'] and result['protectedUnchanged'] and result['technicalSpikeUnchanged']


if __name__=='__main__':main()
