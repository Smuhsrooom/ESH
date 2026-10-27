"""Check ESH-local registration in the native WebView with an inert CLI-shaped file."""
import datetime
import json
import sqlite3
import verify_live as verify


def main():
    verify.RUN = verify.APP / 'evidence' / ('registry-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True, exist_ok=False)
    before = verify.protected()
    fixture = verify.RUN / 'claude.exe'
    fixture.write_bytes(b'inert fixture; never launch')
    result = {'passed': False, 'mockAi': False}
    try:
        data = verify.RUN / 'data'
        gui = verify.launch('registry', data, {'ESH_REGISTRY_PROBE_EXE': str(fixture)})
        status = gui['registryStatus']
        assert status['unsupported'] and status['duplicateRejected'] and status['removed']
        assert status['snapshotUnchanged'] and fixture.exists()
        db = sqlite3.connect(data / 'chat.sqlite')
        assert db.execute('PRAGMA user_version').fetchone()[0] == 6
        assert db.execute('SELECT COUNT(*) FROM agent_instances').fetchone()[0] == 2
        assert db.execute('SELECT COUNT(*) FROM messages').fetchone()[0] == 0
        assert db.execute('SELECT COUNT(*) FROM tool_deliveries').fetchone()[0] == 0
        db.close()
        result.update(passed=True, nativeGui=True, registeredId=status['registeredId'],
                      candidates=status['candidates'], localRowRemoved=True, fixturePreserved=True,
                      conversationUnchanged=True, schemaVersion=6)
    finally:
        result['protectedUnchanged'] = before == verify.protected()
        verify.write('result.json', result)
        print(json.dumps({'evidence': str(verify.RUN), **result}, ensure_ascii=False, indent=2), flush=True)


if __name__ == '__main__':
    main()
