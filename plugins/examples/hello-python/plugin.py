"""pool 別プロセスプラグインのサンプル。依存なし（stdlib のみ）。

プロトコル: stdin/stdout の NDJSON。init → ready、render → done/error。
フレームは RGBA8 ファイル受け渡し。
"""

import json
import sys


def read_msg():
    line = sys.stdin.readline()
    if not line:
        raise EOFError
    return json.loads(line)


def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def main():
    width = height = 0
    while True:
        try:
            msg = read_msg()
        except EOFError:
            return
        kind = msg.get("type")
        if kind == "init":
            width, height = int(msg["width"]), int(msg["height"])
            send({"v": 1, "type": "ready"})
        elif kind == "render":
            try:
                params = msg.get("params", {})
                amount = max(0.0, min(1.0, float(params.get("amount", 1.0))))
                with open(msg["input"]["path"], "rb") as f:
                    data = f.read()
                n = width * height * 4
                data = data[:n]
                if amount >= 1.0:
                    out = bytes(255 - b for b in data)
                elif amount <= 0.0:
                    out = data
                else:
                    out = bytes(
                        round(a * (1.0 - amount) + (255 - a) * amount)
                        for a in data
                    )
                with open(msg["output"]["path"], "wb") as f:
                    f.write(out)
                send({"v": 1, "type": "done"})
            except Exception as e:  # noqa: BLE001 - エラーはプロトコルで返す
                send({"v": 1, "type": "error", "message": str(e)})
        else:
            send({"v": 1, "type": "error", "message": f"unknown: {kind}"})


if __name__ == "__main__":
    main()
