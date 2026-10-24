"""Check all three palettes in the actual Tauri WebView without sending AI turns."""
import json
import verify_live as verify


def main():
    verify.RUN.mkdir(parents=True, exist_ok=False)
    result = {'passed': False}
    try:
        probe = verify.launch('theme', verify.RUN / 'theme-data')
        samples = probe['themeSelection']['samples']
        assert [sample['id'] for sample in samples] == ['slate', 'lilac', 'classic']
        assert probe['themeSelection']['emptyPanelInitiallyClosed']
        result.update(passed=True, samples=samples, restored=probe['themeSelection']['restored'], emptyPanelInitiallyClosed=True)
    except Exception as error:
        result['error'] = repr(error)
        raise
    finally:
        verify.write('result.json', result)
        print(json.dumps({'evidence': str(verify.RUN), **result}, ensure_ascii=True))


if __name__ == '__main__':
    main()
