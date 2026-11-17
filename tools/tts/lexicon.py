"""Text normalization that matches shared/audioKey.js byte for byte.

The app hashes this exact string. Do not expand acronyms or otherwise
rewrite the words: a different string is a different clip, and the player
will not find it.
"""

from __future__ import annotations

import hashlib
import re
import unicodedata

VOICE_ID = "natural-en-us-v1"
_WS = re.compile(r"[\t\n\r\f\v \u00a0]+")


def normalize_spoken(text: str | None) -> str:
    nfc = unicodedata.normalize("NFC", "" if text is None else str(text))
    return _WS.sub(" ", nfc).strip()


def clip_key(voice_id: str, text: str) -> str:
    payload = f"{voice_id}\n{normalize_spoken(text)}"
    return hashlib.sha1(payload.encode("utf-8")).hexdigest()


def clip_path(key: str) -> str:
    return f"audio/clips/{key[:2]}/{key}.mp3"


def choices_readout(question: dict) -> str:
    """Match choicesReadout() in shared/audioKey.js."""
    if question.get("type") == "tf":
        return "True, or false."
    parts: list[str] = []
    for choice in question.get("choices") or []:
        text = normalize_spoken(choice.get("text"))
        if text and not re.search(r"[.!?]$", text):
            text += "."
        key = "" if choice.get("key") is None else str(choice.get("key"))
        parts.append(f"{key}. {text}")
    return " ".join(parts)


def _self_check() -> None:
    key = clip_key(VOICE_ID, "Hello, Drive Study.")
    assert key == "1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb", key
    assert clip_key(VOICE_ID, "Hello,   Drive Study.\n") == key
    assert clip_path(key) == f"audio/clips/13/{key}.mp3"
    question = {
        "type": "mc",
        "choices": [
            {"key": "A", "text": "Common stock"},
            {"key": "B", "text": "Preferred stock."},
        ],
    }
    assert choices_readout(question) == "A. Common stock. B. Preferred stock."
    assert choices_readout({"type": "tf", "choices": []}) == "True, or false."
    print("lexicon self-check ok")


if __name__ == "__main__":
    _self_check()
