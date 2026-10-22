"""Native WebView + actual Codex thread/list; no new model turn or source edits."""
import datetime
import verify_live as verify


def main():
    verify.EXE = verify.APP / 'src-tauri/target/debug/esh-chat-mvp4.exe'
    verify.RUN = verify.APP / 'evidence' / ('import-ui-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True, exist_ok=False)
    protected, spike = verify.protected(), verify.hashes()
    result = {'passed': False}
    try:
        probe = verify.launch('import-ui', verify.RUN / 'data')
        result = {'passed': True, 'mockAi': False, **probe['importUi']}
    finally:
        result['protectedUnchanged'] = protected == verify.protected()
        result['technicalSpikeUnchanged'] = spike == verify.hashes()
        verify.write('result.json', result)
        print(verify.RUN)
        print(result)
    assert result['passed'] and result['protectedUnchanged'] and result['technicalSpikeUnchanged']


if __name__ == '__main__':
    main()
