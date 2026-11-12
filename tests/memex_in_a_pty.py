#!/usr/bin/env python3
"""Run the memex program in a pseudo-terminal, with this script as the terminal.

It answers the start-up probe as a terminal with kitty's keyboard and graphics
protocols would, or as one without them, and sends keys in the kitty keyboard
encoding (CSI code ; modifiers : event u, where event 1 is a press and 3 a
release). It checks what the program does: refuses to start, or writes the
journal entries the keys should make.

Usage: tests/memex_in_a_pty.py <path to the memex binary>
Limits: this is a script playing a terminal, not a real kitty; the escape codes
are from the kitty keyboard protocol's documentation as recalled, and the
test shows only that crossterm and the memex read them as intended.
"""
import fcntl
import json
import os
import pty
import select
import struct
import sys
import tempfile
import termios
import time

KITTY_REPLY = b"\x1b[?0u\x1b_Gi=31;OK\x1b\\\x1b[?62;c"
PLAIN_REPLY = b"\x1b[?62;22c"
ALT = 2  # kitty modifier bit for Alt; the modifiers field is 1 + bits
RIGHT_ALT = 57449  # kitty's code for the Right Alt key


def key(code, mods=0, event=1):
    m = 1 + mods
    return f"\x1b[{code};{m}:{event}u".encode()


def press_release(ch, mods=0):
    return key(ord(ch), mods, 1) + key(ord(ch), mods, 3)


def run(binary, journal, reply, keys, timeout=10):
    """Start the memex in a pty; answer its probe; send keys; return (status, output)."""
    pid, fd = pty.fork()
    if pid == 0:
        os.execv(binary, [binary, journal])
    # A terminal 120 columns by 40 rows, as a window would be.
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    out = b""
    answered = False
    started = False
    sent = False
    deadline = time.time() + timeout
    status = None
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                chunk = b""
            if not chunk:
                # The program closed the terminal: it has ended. Collect its status.
                _, st = os.waitpid(pid, 0)
                status = os.waitstatus_to_exitcode(st)
                break
            out += chunk
        if not answered and b"\x1b[c" in out:
            os.write(fd, reply)
            answered = True
        if not started and b"\x1b[?1049h" in out:  # the alternate screen: it started
            started = True
        if started and not sent:
            time.sleep(0.3)
            for k in keys:
                os.write(fd, k)
                time.sleep(0.02)
            sent = True
        done, st = os.waitpid(pid, os.WNOHANG)
        if done:
            status = os.waitstatus_to_exitcode(st)
            break
    if status is None:
        os.kill(pid, 9)
        os.waitpid(pid, 0)
        status = "killed after timeout"
    try:
        while True:
            r, _, _ = select.select([fd], [], [], 0.1)
            if not r:
                break
            chunk = os.read(fd, 65536)
            if not chunk:
                break
            out += chunk
    except OSError:
        pass
    return status, out


def entries(journal):
    with open(os.path.join(journal, "journal.jsonl"), encoding="utf-8") as f:
        return [json.loads(line) for line in f.read().splitlines()[1:]]


def main():
    binary = os.path.abspath(sys.argv[1])
    failures = 0

    def check(what, ok, detail=""):
        nonlocal failures
        print(("ok:   " if ok else "FAIL: ") + what + ("" if ok else f"\n      {detail}"))
        failures += 0 if ok else 1

    with tempfile.TemporaryDirectory() as d:
        # 1. A terminal without the protocols: refused, saying what is missing.
        status, out = run(binary, os.path.join(d, "plain"), PLAIN_REPLY, [])
        text = out.decode("utf-8", "replace")
        check("a terminal without the protocols is refused", status == 1, f"status {status}")
        check("the refusal names both protocols",
              "kitty keyboard protocol" in text and "kitty graphics protocol" in text, text[-300:])
        check("nothing is written when refused", not os.path.exists(os.path.join(d, "plain", "journal.jsonl")))

        # 2. A kitty terminal: type, Leap with Right Alt held, quit with F10.
        j = os.path.join(d, "kitty")
        keys = [press_release(c) for c in "Hello"]
        keys += [b"\r"]  # Enter, legacy form
        keys += [press_release(c) for c in "World"]
        keys += [key(RIGHT_ALT, ALT, 1)]  # hold Leap forward
        keys += [press_release(c, ALT) for c in "hel"]
        keys += [key(RIGHT_ALT, 0, 3)]  # release it
        keys += [b"\x1b[21~"]  # F10: quit
        status, out = run(binary, j, KITTY_REPLY, keys)
        check("with the protocols it starts and quits cleanly", status == 0, f"status {status}: {out[-300:]!r}")
        es = entries(j) if os.path.exists(os.path.join(j, "journal.jsonl")) else []
        texts = [e["body"].get("add") for e in es if e["kind"] == "edit" and e["body"].get("op") == "text"]
        check("typed text reached the journal", "Hello" in texts and "World" in texts, texts)
        leaps = [e["body"] for e in es if e["kind"] == "view" and e["body"].get("op") == "leap"]
        check("a held Right Alt made one Leap, ended by its release",
              len(leaps) == 1 and leaps[0]["pattern"] == "hel", leaps)
        first = next((e["seq"] for e in es if e["kind"] == "edit" and e["body"].get("op") == "create"), None)
        check("the Leap went back to the first item", leaps and leaps[0].get("found") == first, (leaps, first))

    print(f"{failures} failed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
