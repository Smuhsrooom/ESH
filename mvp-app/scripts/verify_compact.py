"""Real GUI check for the manual context-compaction button (one short Codex turn, no mock AI)."""
import datetime
import json
import sqlite3
import verify_live as verify


def main():
    verify.RUN = verify.APP / 'evidence' / ('compact-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True, exist_ok=False)
    protected, spike = verify.protected(), verify.hashes()
    result = {'passed': False, 'mockAi': False}
    try:
        data = verify.RUN / 'data'
        probe = verify.launch('compact', data)
        compact = probe['compact']
        db = sqlite3.connect(data / 'chat.sqlite')
        rows = [{'sequence': n, 'kind': k, 'payload': json.loads(p)} for n, k, p in
                db.execute('SELECT sequence,kind,payload FROM audit ORDER BY sequence')]
        verify.write('audit.json', rows)
        assert db.execute('PRAGMA user_version').fetchone()[0] == 5
        db.close()

        thread = compact['threadId']
        assert [r for r in rows if r['kind'] == 'compact_requested'], 'manual compact was not requested'
        requests = [r for r in rows if r['kind'] == 'codex_tx' and r['payload'].get('method') == 'thread/compact/start']
        assert len(requests) == 1 and requests[0]['payload']['params']['threadId'] == thread, requests
        started = [r for r in rows if r['kind'] == 'codex_rx' and r['payload'].get('method') == 'item/started'
                   and r['payload']['params']['item']['type'] == 'contextCompaction'
                   and r['payload']['params']['threadId'] == thread]
        completed = [r for r in rows if r['kind'] == 'codex_rx' and r['payload'].get('method') == 'item/completed'
                     and r['payload']['params']['item']['type'] == 'contextCompaction'
                     and r['payload']['params']['threadId'] == thread]
        assert len({r['payload']['params']['item']['id'] for r in started}) == 1
        assert {r['payload']['params']['item']['id'] for r in started} == {r['payload']['params']['item']['id'] for r in completed}
        assert any(r['kind'] == 'compact_completed' for r in rows), 'compact_completed audit missing'

        assert compact['compactionCountAfter'] == compact['compactionCountBefore'] + 1, compact
        assert 'compacting' in compact['observed'] and 'completed' in compact['observed'], compact['observed']
        assert compact['threadUnchanged'] and compact['messageCountUnchanged'] and compact['buttonEnabledAfter']
        context = next(c for c in probe['snapshot']['contexts'] if c['sessionId'] == compact['sessionId'])
        assert context['compaction'] == 'completed' and context['compactionCount'] == compact['compactionCountAfter']
        result.update(passed=True, threadId=thread,
                      compactionCountBefore=compact['compactionCountBefore'],
                      compactionCountAfter=compact['compactionCountAfter'],
                      guiStates=compact['observed'],
                      compactRequests=len(requests),
                      compactionItemIds=len({r['payload']['params']['item']['id'] for r in started}),
                      messagesPreserved=compact['messageCount'],
                      sameThread=True, historyUnchanged=True)
    finally:
        result['protectedUnchanged'] = protected == verify.protected()
        result['technicalSpikeUnchanged'] = spike == verify.hashes()
        verify.write('result.json', result)
        print(verify.RUN)
        print(result)
    assert result['passed'] and result['protectedUnchanged'] and result['technicalSpikeUnchanged']


if __name__ == '__main__':
    main()
