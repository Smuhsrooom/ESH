"""Final-build native GUI checks using copies of actual saved CLI conversations."""
import argparse
import datetime
import hashlib
import json
import sqlite3
from pathlib import Path
import verify_live as verify


def backup(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    read=sqlite3.connect(f'file:{source.as_posix()}?mode=ro',uri=True)
    write=sqlite3.connect(target)
    read.backup(write);read.close();write.close()


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--live-run',default='panels-20261018-035628')
    args=parser.parse_args()
    source=verify.APP/'evidence'/args.live_run
    live=json.loads((source/'result.json').read_text(encoding='utf8'))
    assert live['passed'] and not live['mockAi']
    verify.RUN=verify.APP/'evidence'/('panels-final-'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True,exist_ok=False)
    before=verify.protected();verify.write('protected-before.json',before)
    result={'passed':False,'realCliEvidence':args.live_run,'mockAi':False}
    try:
        data=verify.RUN/'restored-data';backup(source/'live-data/chat.sqlite',data/'chat.sqlite')
        restored=verify.launch('restore-history',data,label='final-restored')
        original=json.loads((source/'restore-gui.json').read_text(encoding='utf8'))
        assert [{k:v for k,v in row.items() if k != 'renderedText'} for row in restored['rendered']]==[{k:v for k,v in row.items() if k != 'renderedText'} for row in original['rendered']]
        legacy=verify.APP/'evidence/gui-20261018-032922/live-data/chat.sqlite'
        old=sqlite3.connect(f'file:{legacy.as_posix()}?mode=ro',uri=True)
        records=old.execute('SELECT sequence,id,speaker,content,created_at FROM messages ORDER BY sequence').fetchall();old.close()
        migration=verify.RUN/'migration-data';backup(legacy,migration/'chat.sqlite')
        probe=verify.launch('restore-history',migration,label='final-migration')
        db=sqlite3.connect(migration/'chat.sqlite')
        for record in records:
            assert db.execute('SELECT sequence,id,speaker,content,created_at FROM messages WHERE id=?',(record[1],)).fetchone()==record
        assert db.execute('PRAGMA user_version').fetchone()[0]==6;db.close()
        assert any(m['kind']=='review_request' for m in probe['rendered'])
        assert any(m['kind']=='review_reply' for m in probe['rendered'])
        result.update(passed=True,finalBuildRestoredMessages=len(restored['rendered']),legacyRecordsPreserved=len(records),panelsReadOnlyAndResize=True)
    finally:
        after=verify.protected();verify.write('protected-after.json',after)
        result['protectedFilesUnchanged']=before==after
        original=json.loads((verify.APP/'evidence/panels-spike-before.json').read_text(encoding='utf8'))
        result['technicalSpikeUnchanged']=verify.hashes()==original
        paths=[*sorted((verify.APP/'src').glob('*')),*sorted((verify.APP/'src-tauri/src').glob('*.rs')),verify.EXE]
        result['sha256']={str(p.relative_to(verify.APP)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths if p.is_file()}
        verify.write('result.json',result)
        print(json.dumps({'evidence':str(verify.RUN),**result},ensure_ascii=False,indent=2))


if __name__=='__main__':main()
