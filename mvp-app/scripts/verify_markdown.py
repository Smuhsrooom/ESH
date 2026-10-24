"""Synthetic rendering/security fixtures in the actual Tauri WebView; not CLI success."""
import datetime
import sqlite3
import verify_live as verify


def main():
    verify.RUN = verify.APP / 'evidence' / ('markdown-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    verify.RUN.mkdir(parents=True, exist_ok=False)
    protected, spike = verify.protected(), verify.hashes()
    result = {'passed': False, 'syntheticRenderingFixture': True, 'newAiCalls': False}
    try:
        data = verify.RUN/'data'
        verify.launch('restore',data,label='initialize')
        content = '''# 마크다운 검증

**강조**와 `inline()` 코드, ~~삭제선~~

1. 첫 항목
2. 두 번째 항목

> 인용문

| 항목 | 값 |
| --- | --- |
| 상태 | 완료 |

```typescript
const wide = "''' + ('long-line-'*60) + '''";
```

[정상 링크](https://example.com/docs)
[위험 링크](javascript:alert(1))
![외부 이미지](https://example.com/pixel.png)

<script>window.__eshMarkdownInjected = true;</script>
'''
        fixtures=[('markdown-user','user','# 사용자 원문\n**편집한 글 그대로**','main','user_message'),
                  ('markdown-lead','codex',content,'main','answer'),
                  ('markdown-arch','antigravity','## Arch 검토\n\n- [x] 확인 완료\n- [ ] 대기\n\n| 항목 | 판단 | 근거 |\n| --- | --- | --- |\n| 긴 코드 | 완료 | 길게_이어지는_식별자_줄바꿈_검증_내용_입니다 |','collaboration','review_reply')]
        db=sqlite3.connect(data/'chat.sqlite')
        for id,speaker,text,channel,kind in fixtures:
            db.execute("INSERT INTO messages(id,speaker,content,created_at,delivery,version,is_error,channel,kind,room_id,chat_session_id,collaboration_id) VALUES(?,?,?,'2026-10-18T00:00:00Z','delivered',0,0,?,?,'default','default-session','rendering-fixture')",(id,speaker,text,channel,kind))
        db.commit();db.close()
        probe=verify.launch('markdown',data)
        assert probe['markdownUi']['syntheticRenderingFixture']
        db=sqlite3.connect(data/'chat.sqlite')
        for id,_,text,_,_ in fixtures:
            assert db.execute('SELECT content FROM messages WHERE id=?',(id,)).fetchone()[0]==text
        db.close()
        result.update(passed=True,**probe['markdownUi'],sourceMarkdownUnchanged=True)
    finally:
        result['protectedUnchanged']=protected==verify.protected()
        result['technicalSpikeUnchanged']=spike==verify.hashes()
        verify.write('result.json',result)
        print(verify.RUN)
        print(result)
    assert result['passed'] and result['protectedUnchanged'] and result['technicalSpikeUnchanged']


if __name__=='__main__':
    main()
