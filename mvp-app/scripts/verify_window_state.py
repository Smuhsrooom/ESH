"""Check normal and maximized window restore using an isolated ESH data directory."""
import ctypes
import datetime
import json
import os
import subprocess
import time
from ctypes import wintypes
from pathlib import Path

APP = Path(__file__).resolve().parents[1]
RUN = APP / 'evidence' / ('window-restore-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
EXE = APP / 'src-tauri' / 'target' / 'debug' / 'esh-chat-mvp4.exe'
USER32 = ctypes.windll.user32
USER32.EnumWindows.argtypes = [ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM), wintypes.LPARAM]
USER32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
USER32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
USER32.MoveWindow.argtypes = [wintypes.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.BOOL]
USER32.SendMessageTimeoutW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM, wintypes.UINT, wintypes.UINT, ctypes.POINTER(ctypes.c_size_t)]
try:
    USER32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
except AttributeError:
    USER32.SetProcessDPIAware()


def window_for(pid):
    found = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def visit(hwnd, _):
        owner = wintypes.DWORD()
        USER32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and USER32.IsWindowVisible(hwnd):
            title = ctypes.create_unicode_buffer(256)
            bounds = wintypes.RECT()
            USER32.GetWindowTextW(hwnd, title, 256)
            USER32.GetWindowRect(hwnd, ctypes.byref(bounds))
            if title.value.startswith('ESH') and bounds.right - bounds.left >= 900 and bounds.bottom - bounds.top >= 600:
                found.append(hwnd)
        return True

    USER32.EnumWindows(visit, 0)
    return found[0] if found else None


def launch():
    env = os.environ.copy()
    env['ESH_DATA_DIR'] = str(RUN / 'data')
    env.pop('ESH_GUI_PROBE', None)
    process = subprocess.Popen([str(EXE)], env=env)
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f'ESH exited before showing a window: {process.returncode}')
        hwnd = window_for(process.pid)
        if hwnd:
            return process, hwnd
        time.sleep(.1)
    process.terminate()
    raise TimeoutError('ESH window did not become visible')


def rect(hwnd):
    value = wintypes.RECT()
    assert USER32.GetWindowRect(hwnd, ctypes.byref(value))
    return [value.left, value.top, value.right - value.left, value.bottom - value.top]


def close(process, hwnd):
    response = ctypes.c_size_t()
    assert USER32.SendMessageTimeoutW(hwnd, 0x0010, 0, 0, 2, 5000, ctypes.byref(response))
    process.wait(timeout=40)
    assert process.returncode == 0


def similar(actual, expected):
    return all(abs(a - b) <= 16 for a, b in zip(actual, expected))


def main():
    RUN.mkdir(parents=True, exist_ok=False)
    result = {'passed': False, 'dataDirectory': str(RUN / 'data')}
    process = None
    try:
        process, hwnd = launch()
        time.sleep(1)
        initial = rect(hwnd)
        width = max(900, min(1120, USER32.GetSystemMetrics(0) - 150))
        height = max(600, min(720, USER32.GetSystemMetrics(1) - 150))
        assert USER32.MoveWindow(hwnd, 100, 80, width, height, True)
        time.sleep(.8)
        wanted = rect(hwnd)
        result.update(initial=initial, moved=wanted)
        assert abs(wanted[2] - width) <= 16 and abs(wanted[3] - height) <= 16, (width, height, wanted)
        close(process, hwnd)
        saved = json.loads((RUN / 'data' / 'window-state.json').read_text(encoding='utf8'))
        assert not saved['maximized'] and similar([saved['x'], saved['y']], wanted[:2])
        assert saved['width'] <= wanted[2] and saved['height'] <= wanted[3]

        process, hwnd = launch()
        restored = rect(hwnd)
        assert not USER32.IsZoomed(hwnd) and similar(restored, wanted), (wanted, restored)
        USER32.ShowWindow(hwnd, 3)
        time.sleep(.4)
        assert USER32.IsZoomed(hwnd)
        close(process, hwnd)
        saved_max = json.loads((RUN / 'data' / 'window-state.json').read_text(encoding='utf8'))
        assert saved_max['maximized']

        process, hwnd = launch()
        maximized_after_restart = bool(USER32.IsZoomed(hwnd))
        close(process, hwnd)
        assert maximized_after_restart
        result.update(passed=True, normalBefore=wanted, normalAfter=restored, maximizedAfterRestart=True)
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
        (RUN / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf8')
        print(json.dumps({'evidence': str(RUN), **result}, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
