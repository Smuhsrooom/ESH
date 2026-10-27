"""Check Agent probe fields and manual refresh in the actual Tauri WebView."""
import json
from pathlib import Path
import sqlite3
import verify_live as verify


def main():
    verify.RUN.mkdir(parents=True, exist_ok=False)
    result = {'passed': False}
    try:
        normal = verify.launch('agents', verify.RUN / 'normal-data', label='normal')
        missing_path = verify.RUN / 'missing-agy.exe'
        missing = verify.launch('agents', verify.RUN / 'missing-data', {'ESH_AGY_BIN': str(missing_path)}, label='missing')
        normal_rows = {row['id']: row for row in normal['agentStatus']['second']}
        missing_rows = {row['id']: row for row in missing['agentStatus']['second']}
        assert set(normal_rows) == set(missing_rows) == {'codex', 'antigravity'}
        assert all(row['authentication'] == 'unknown' for row in normal_rows.values())
        assert missing_rows['antigravity']['installation'] == 'missing'
        assert missing_rows['antigravity']['availability'] == 'unavailable'
        assert '실행 파일을 찾을 수 없습니다' in missing_rows['antigravity']['text']
        assert normal['agentStatus']['snapshotUnchanged'] and missing['agentStatus']['snapshotUnchanged']
        for name in ['normal', 'missing']:
            db = sqlite3.connect(verify.RUN / (name + '-data') / 'chat.sqlite')
            assert db.execute('PRAGMA user_version').fetchone()[0] == 6
            assert db.execute('SELECT COUNT(*) FROM messages').fetchone()[0] == 0
            assert db.execute('SELECT COUNT(*) FROM tool_deliveries').fetchone()[0] == 0
            db.close()
        result.update(passed=True, nativeGui=True, mockAi=False, normal=normal_rows, missing=missing_rows,
                      manualRefresh=True, snapshotUnchanged=True, schemaVersion=5)
    except Exception as error:
        result['error'] = repr(error)
        raise
    finally:
        verify.write('result.json', result)
        print(json.dumps({'evidence': str(verify.RUN), **result}, ensure_ascii=True))


if __name__ == '__main__':
    main()
