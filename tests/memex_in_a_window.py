#!/usr/bin/env python3
"""Run the memex program in its window, with this script typing into it.

Under X11 it starts an X server with no screen (Xvfb) and types with
xdotool by key code, since xdotool's key names press Left Alt with Right
Alt, and wraps later keys in Alt. Under Wayland it starts a headless sway
and types and clicks with the keyboard and pointer programs (crates/tools),
which keep one device each on the seat and answer when each command is
done; it clicks at a scale of 2, so a click is found in points, not pixels.
Focus is taken from the window by focusing the root window (X11) or an
empty workspace (Wayland). It checks what the program does: the journal
entries the keys should make, and its exit status.

It waits on events, never the clock: Xvfb and sway say when they are
ready, xdotool and sway when the window is there and has focus, the
device programs when each command is done, Linux (inotify) when the
journal changes, the screen program when the screen shows the window,
and a process's pidfd when it ends. Each wait has a deadline, so a fault
fails a check instead of hanging the test.

Usage: tests/memex_in_a_window.py <path to the memex binary>
Needs: Xvfb, xdotool and sway (the Nix shell has them), the window's
libraries and drivers (the Nix shell sets them), and the keyboard, pointer
and screen programs built beside the memex (make test-window builds them).
"""
import ctypes
import json
import os
import select
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time

# X key codes (evdev, Xvfb's default keymap).
RETURN, F10, F12, INSERT, RIGHT_ALT, LEFT_CTRL = 36, 76, 96, 118, 108, 37
# A cell is 9 by 18 pixels or so; the first item is on the fourth row.
FIRST_ITEM = (40, 3 * 18 + 9)


def typed(text):
    return ["type", "--delay", "20", text]


def key(code):
    return ["key", str(code)]


def held(code, text):
    """Hold a key by its code while typing text."""
    return [["keydown", str(code)], typed(text), ["keyup", str(code)]]


def command(line):
    return held(LEFT_CTRL, line)


def readable(source, process=None, deadline=None):
    """Whether a file or socket has something to read (or has ended)
    before the process ends or the deadline passes: one wait on all three,
    as events."""
    pidfd = os.pidfd_open(process.pid) if process is not None and process.poll() is None else None
    waits = [source] if pidfd is None else [source, pidfd]
    left = None if deadline is None else max(deadline - time.time(), 0)
    try:
        return source in select.select(waits, [], [], left)[0]
    finally:
        if pidfd is not None:
            os.close(pidfd)


LIBC = ctypes.CDLL(None, use_errno=True)
IN_MODIFY, IN_CLOSE_WRITE, IN_MOVED_TO, IN_CREATE = 0x2, 0x8, 0x80, 0x100


class Changes:
    """Changes to the files in a directory, as Linux reports them (inotify)."""

    def __init__(self, directory):
        self.fd = LIBC.inotify_init1(os.O_NONBLOCK | os.O_CLOEXEC)
        mask = IN_MODIFY | IN_CLOSE_WRITE | IN_MOVED_TO | IN_CREATE
        if self.fd < 0 or LIBC.inotify_add_watch(self.fd, directory.encode(), mask) < 0:
            raise OSError(ctypes.get_errno(), f"cannot watch {directory}")

    def until(self, holds, process, deadline):
        """Whether holds() comes true before the process ends or the
        deadline passes; asked again at each change."""
        while not holds():
            if not readable(self.fd, process, deadline):
                return holds()
            os.read(self.fd, 65536)
        return True

    def close(self):
        os.close(self.fd)


def wait_for_window(env, process, timeout=30):
    """The memex window's id, once it is mapped (xdotool waits for it);
    none if the program ends first, or the time is up."""
    search = subprocess.Popen(["xdotool", "search", "--sync", "--name", "^memex$"], env=env,
                              stdout=subprocess.PIPE, text=True)
    found = readable(search.stdout, process, time.time() + timeout)
    window = (search.stdout.readline().split() or [None])[0] if found else None
    search.kill()
    return window


def run_x11(binary, display, journal, actions, args=(), kill_when=None, timeout=20):
    """Run the memex on a journal, type the actions into its window, and
    return its exit status and what it wrote to stderr. With kill_when,
    the memex is killed, as a crash would end it, once kill_when() holds
    of its journal, or after 2 seconds."""
    env = dict(os.environ, DISPLAY=display)
    env.pop("WAYLAND_DISPLAY", None)
    p = subprocess.Popen([binary, *args, journal], env=env,
                         stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    w = wait_for_window(env, p)
    if w is None:
        if p.poll() is None:
            p.kill()
        _, err = p.communicate()
        return (p.returncode if p.returncode >= 0 else None), err.decode("utf-8", "replace")
    subprocess.run(["xdotool", "windowfocus", "--sync", w], env=env)
    root = subprocess.run(["xdotool", "search", "--maxdepth", "0", "--name", ""], env=env,
                          capture_output=True, text=True).stdout.split()[:1]
    places = {"%memex": w, "%root": root[0] if root else ""}
    changes = Changes(journal) if kill_when is not None else None
    for a in actions:
        subprocess.run(["xdotool", *[places.get(x, x) for x in a]], env=env)
    if changes is not None:
        changes.until(kill_when, p, time.time() + 2)
        changes.close()
        p.send_signal(signal.SIGKILL)
    try:
        _, err = p.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        p.kill()
        _, err = p.communicate()
        return None, "timed out"
    return p.returncode, err.decode("utf-8", "replace")


def entries(journal):
    path = os.path.join(journal, "journal.jsonl")
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as f:
        return [e for e in map(json.loads, f) if "kind" in e]


def texts(es):
    return [e["body"].get("add") for e in es if e["kind"] == "edit" and e["body"].get("op") == "text"]


def items(es):
    return [e["seq"] for e in es if e["kind"] == "edit" and e["body"].get("op") == "create"]


def leaps(es):
    return [e["body"] for e in es if e["kind"] == "view" and e["body"].get("op") == "leap"]


def flat(*parts):
    out = []
    for p in parts:
        out += p if p and isinstance(p[0], list) else [p]
    return out


def x11_checks(binary, d, check):
    # Xvfb writes its display's number to -displayfd once it takes clients.
    ready, told_fd = os.pipe()
    xvfb = subprocess.Popen(["Xvfb", ":91", "-screen", "0", "1400x900x24", "-displayfd", str(told_fd)],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, pass_fds=(told_fd,))
    os.close(told_fd)
    with os.fdopen(ready) as r:
        r.readline()
    try:
        x11_typing(binary, d, check)
        x11_remapped(binary, d, check)
        x11_mouse(binary, d, check)
        x11_idle_and_reasoning(binary, d, check)
        x11_focus_lost(binary, d, check)
    finally:
        xvfb.kill()


def x11_typing(binary, d, check):
    # Type, Leap with Right Alt held, quit with F10.
    j = os.path.join(d, "typing")
    actions = flat(typed("Hello"), key(RETURN), typed("World"), held(RIGHT_ALT, "hel"), key(F10))
    status, err = run_x11(binary, ":91", j, actions)
    check("it opens a window and quits cleanly with F10", status == 0, f"status {status}: {err[-300:]}")
    es = entries(j)
    check("typed text reached the journal", "Hello" in texts(es) and "World" in texts(es), texts(es))
    ls = leaps(es)
    check("a held Right Alt made one Leap forward, ended by its release",
          len(ls) == 1 and ls[0]["pattern"] == "hel" and ls[0]["forward"] is True, ls)
    check("the Leap went back to the first item", ls and ls[0].get("found") == items(es)[0], (ls, items(es)))


def x11_remapped(binary, d, check):
    # A configuration that moves Leap forward to F12, unbinds F10 and binds
    # Insert to quit.
    conf = os.path.join(d, "remapped.toml")
    with open(conf, "w", encoding="utf-8") as f:
        f.write('[quasimodes]\nleap_forward = "F12"\n[keys]\nF10 = ""\nInsert = "quit"\n')
    j = os.path.join(d, "remapped")
    actions = flat(typed("Hello"), key(RETURN), typed("World"), held(F12, "hel"),
                   key(F10), typed("Z"), key(INSERT))
    status, err = run_x11(binary, ":91", j, actions, args=["--config", conf])
    check("a remapped key quits", status == 0, f"status {status}: {err[-300:]}")
    es = entries(j)
    ls = leaps(es)
    check("a held F12 made one Leap, ended by its release", len(ls) == 1 and ls[0]["pattern"] == "hel", ls)
    check("the Leap pattern was not typed as text", "hel" not in "".join(t or "" for t in texts(es)), texts(es))
    check("F10, unbound, did not quit: a key typed after it was written",
          any("Z" in (t or "") for t in texts(es)), texts(es))


def x11_mouse(binary, d, check):
    conf = os.path.join(d, "mouse.toml")
    with open(conf, "w", encoding="utf-8") as f:
        f.write("[mouse]\nenabled = true\n")
    click = [["mousemove", str(FIRST_ITEM[0]), str(FIRST_ITEM[1])], ["click", "1"]]
    actions = flat(typed("Hello"), key(RETURN), typed("World"), click, typed("Z"), key(F10))
    for name, args, takes in [("by default", [], False), ("with [mouse] enabled", ["--config", conf], True),
                              ("with --no-mouse over [mouse] enabled", ["--config", conf, "--no-mouse"], False),
                              ("with --mouse", ["--mouse"], True)]:
        j = os.path.join(d, "mouse-" + str(abs(hash(name))))
        status, err = run_x11(binary, ":91", j, actions, args=args)
        es = entries(j)
        first = items(es)[0] if items(es) else None
        into = [(e["body"].get("item"), e["body"].get("add")) for e in es
                if e["kind"] == "edit" and e["body"].get("op") == "text"]
        moved = any(i == first and "Z" in (t or "") for i, t in into)
        check(f"{name} a click on the first item {'puts' if takes else 'does not put'} the next key there",
              status == 0 and moved == takes, (status, first, into, err[-200:]))
    status, _ = run_x11(binary, ":91", os.path.join(d, "both"), [], args=["--mouse", "--no-mouse"])
    check("--mouse with --no-mouse is a usage error", status == 2, f"status {status}")


def x11_idle_and_reasoning(binary, d, check):
    # Text typed and left for a second is written, so a crash after it
    # loses nothing.
    j = os.path.join(d, "idle")
    status, _ = run_x11(binary, ":91", j, [typed("Kept")], kill_when=lambda: "Kept" in texts(entries(j)))
    check("text left for a second is written before a crash", "Kept" in texts(entries(j)), texts(entries(j)))
    # The memex reasons after each change, unasked: two dates for one event
    # and a rule against it make a violation, which it observes. The
    # commands are typed with Left Ctrl held, parentheses and all.
    j = os.path.join(d, "reasons")
    actions = flat(typed("Buran"), command("fact date(buran, 1988)"), command("fact date(buran, 1989)"),
                   command("constraint one date: false :- date(E, D1), date(E, D2), D1 \\= D2."), key(F10))
    status, err = run_x11(binary, ":91", j, actions)
    es = entries(j)
    facts = [e["body"].get("facts") for e in es if e["kind"] == "edit" and e["body"].get("op") == "facts"]
    check("commands typed with Left Ctrl held keep their parentheses", len(facts) == 2, (facts, err[-300:]))
    seen = [e["body"].get("violations") for e in es if e["kind"] == "view" and e["body"].get("op") == "observed"]
    check("a change is reasoned over and its violation observed",
          status == 0 and any(seen), (status, seen))


def x11_focus_lost(binary, d, check):
    # Right Alt held, then the keyboard taken by another window, where the
    # key is released, as Alt+Tab does: back in the window, Leap must not
    # still be held, so Z is text and F10 quits.
    j = os.path.join(d, "focus")
    away = [["keydown", str(RIGHT_ALT)], ["windowfocus", "--sync", "%root"],
            ["keyup", str(RIGHT_ALT)], ["windowfocus", "--sync", "%memex"]]
    status, err = run_x11(binary, ":91", j, flat(typed("Hello"), away, typed("Z"), key(F10)))
    es = entries(j)
    check("after focus is lost with Right Alt held, keys are typed again and F10 quits",
          status == 0 and "Z" in texts(es) and not leaps(es), (status, texts(es), leaps(es), err[-200:]))


def started_sway(d, scale, timeout=30):
    """A headless sway with one output at a scale; its process, and the
    environment its clients need. sway runs its config's exec commands
    once it takes clients: one writes the two names a client needs."""
    run_dir = tempfile.mkdtemp(dir=d)
    os.chmod(run_dir, 0o700)
    told_path, script = os.path.join(run_dir, "ready"), os.path.join(run_dir, "ready.sh")
    os.mkfifo(told_path)
    with open(script, "w", encoding="utf-8") as f:
        f.write(f'#!/bin/sh\necho "$WAYLAND_DISPLAY $SWAYSOCK" > {told_path}\n')
    os.chmod(script, 0o700)
    cfg = os.path.join(run_dir, "sway.cfg")
    with open(cfg, "w", encoding="utf-8") as f:
        f.write(f"output HEADLESS-1 resolution 1600x1000 scale {scale}\nxwayland disable\n"
                f"default_border none\nexec {script}\n")
    env = dict(os.environ, XDG_RUNTIME_DIR=run_dir, WLR_BACKENDS="headless",
               WLR_LIBINPUT_NO_DEVICES="1", WLR_RENDERER="pixman")
    env.pop("DISPLAY", None)
    # Both ends held open here: the read end does not see an end before
    # sway writes.
    told = os.open(told_path, os.O_RDONLY | os.O_NONBLOCK)
    keep = os.open(told_path, os.O_WRONLY)
    sway = subprocess.Popen(["sway", "-c", cfg], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    names = os.read(told, 4096).decode().split() if readable(told, sway, time.time() + timeout) else []
    os.close(told)
    os.close(keep)
    env["WAYLAND_DISPLAY"], env["SWAYSOCK"] = (names + ["", ""])[:2]
    return sway, env


def device(binary, name, env):
    """A device program (crates/tools: keyboard, pointer), started beside
    the memex's binary, and whether it said its device is on the seat."""
    program = os.path.join(os.path.dirname(binary), name)
    hand = subprocess.Popen([program], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    return hand, hand.stdout.readline().strip() == "ready"


def told(hand, *commands):
    """Give a device program commands, one by one, each once the last is
    done; whether each was done. A program that has ended does none."""
    answers = []
    try:
        for c in commands:
            hand.stdin.write(c + "\n")
            hand.stdin.flush()
            answers.append(hand.stdout.readline().strip())
    except BrokenPipeError:
        return False
    return answers == ["done"] * len(commands)


def finished(hand):
    """End a device program's input, so it takes its device off the seat
    and ends; it may have ended already."""
    try:
        hand.stdin.close()
    except BrokenPipeError:
        pass
    hand.wait(timeout=10)


def sway_ipc(path):
    """A connection to sway's IPC socket, subscribed to window events once
    sway has said it is: from then, no event is missed. None if there is
    no sway to connect to, or it does not say so."""
    ipc = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        ipc.connect(path)
    except OSError:
        ipc.close()
        return None
    body = b'["window"]'
    ipc.sendall(b"i3-ipc" + struct.pack("=II", len(body), 2) + body)
    reply = sway_message(ipc, None, time.time() + 10)
    return ipc if reply and json.loads(reply).get("success") else None


def sway_message(ipc, process, deadline):
    """The body of sway's next message on an IPC connection; none if none
    comes before the process ends or the deadline passes."""
    header = received(ipc, 14, process, deadline)
    if header is None:
        return None
    length, _ = struct.unpack("=II", header[6:])
    return received(ipc, length, process, deadline)


def received(ipc, count, process, deadline):
    """Exactly count bytes from a socket; none if they do not all come
    before the process ends or the deadline passes."""
    data = b""
    while len(data) < count:
        chunk = ipc.recv(count - len(data)) if readable(ipc, process, deadline) else b""
        if not chunk:
            return None
        data += chunk
    return data


def launched_on_sway(binary, env, args, timeout=30):
    """The memex started under sway, and whether its window took the
    keyboard. Window events are followed from before it starts, so the
    focus is not missed, until it comes or the time is up."""
    ipc = sway_ipc(env["SWAYSOCK"])
    p = subprocess.Popen([binary, *args], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    deadline = time.time() + timeout
    focused = False
    while ipc and not focused:
        body = sway_message(ipc, p, deadline)
        if body is None:
            break
        event = json.loads(body)
        focused = event.get("change") == "focus" and event.get("container", {}).get("name") == "memex"
    if ipc:
        ipc.close()
    return p, focused


def ended(p):
    """A program's exit status and what it wrote to stderr, killed if it
    has not ended in 20 seconds."""
    try:
        _, err = p.communicate(timeout=20)
        return p.returncode, err.decode("utf-8", "replace")
    except subprocess.TimeoutExpired:
        p.kill()
        return None, "timed out"


# Evdev key codes, as the keyboard program sends them.
EV_RETURN, EV_F10, EV_F12, EV_RIGHT_ALT = 28, 68, 88, 100


def wayland_checks(binary, d, check):
    """Keys under a headless Wayland compositor, from the keyboard
    program, by their places on the keyboard as a real keyboard sends
    them."""
    sway, env = started_sway(d, 1)
    keys, ready = device(binary, "keyboard", env)
    try:
        j = os.path.join(d, "wayland")
        p, focused = launched_on_sway(binary, env, [j])
        typed_in = told(keys, "type Hello", f"key {EV_RETURN}", "type World",
                        f"down {EV_RIGHT_ALT}", "type hel", f"up {EV_RIGHT_ALT}", f"key {EV_F10}")
        status, err = ended(p)
        es = entries(j)
        check("under Wayland it opens a window and quits with F10",
              ready and focused and typed_in and status == 0, (ready, focused, typed_in, status, err[-300:]))
        check("under Wayland typed text reached the journal", "Hello" in texts(es) and "World" in texts(es), texts(es))
        ls = leaps(es)
        check("under Wayland a held Right Alt made one Leap forward, ended by its release",
              len(ls) == 1 and ls[0]["pattern"] == "hel" and ls[0]["forward"] is True, ls)
        wayland_focus_lost(binary, d, env, keys, check)
    finally:
        finished(keys)
        sway.kill()


def wayland_focus_lost(binary, d, env, keys, check):
    """Right Alt held while sway moves to an empty workspace, and released
    there: back in the window, Leap must not still be held, so Z is text
    and F10 quits. sway answers a command once it has acted on it."""
    j = os.path.join(d, "wayland-focus")
    p, focused = launched_on_sway(binary, env, [j])
    sway = lambda *words: subprocess.run(["swaymsg", "-q", *words], env=env).returncode == 0
    steps = [told(keys, "type Hello", f"down {EV_RIGHT_ALT}"), sway("workspace", "2"),
             told(keys, f"up {EV_RIGHT_ALT}"), sway("workspace", "1"), told(keys, "type Z", f"key {EV_F10}")]
    status, err = ended(p)
    es = entries(j)
    check("under Wayland, after focus is lost with Right Alt held, keys are typed again and F10 quits",
          focused and all(steps) and status == 0 and "Z" in texts(es) and not leaps(es),
          (focused, steps, status, texts(es), leaps(es), err[-200:]))


# The first item's row of the outline pane, in the output's own units, and
# the memex's ink (crate::gui::paint::INK).
FIRST_ITEM_ROW = (0, FIRST_ITEM[1] - 9, 300, 14)
INK = "dcdcdc"


def on_screen(binary, env, process, region, colour, timeout=30):
    """Whether the screen shows a colour in a region before the process
    ends or the time is up: the screen program waits on the compositor."""
    program = os.path.join(os.path.dirname(binary), "screen")
    eye = subprocess.Popen([program, *map(str, region), colour], env=env, stdout=subprocess.PIPE, bufsize=0)
    shown = readable(eye.stdout, process, time.time() + timeout) and eye.stdout.readline() == b"shown\n"
    eye.kill()
    return shown


def wayland_click(binary, d, check):
    """A click at a scale of 2: the window is told of the pointer in
    pixels, twice the points the cells are measured in."""
    sway, env = started_sway(d, 2)
    keys, keyboard_ready = device(binary, "keyboard", env)
    hand, pointer_ready = device(binary, "pointer", env)
    try:
        j = os.path.join(d, "wayland-click")
        p, focused = launched_on_sway(binary, env, ["--mouse", j])
        typed_in = told(keys, "type Hello", f"key {EV_RETURN}", "type World")
        # A click acts on what the window shows, and the window draws a
        # while after the keys come: sway shows it some 20 ms after it
        # reports it focused, and the keys and a click can all come before
        # its next frame. So the click waits, as a person would, until the
        # screen shows the first item's row in the memex's ink (its text,
        # not the dimmed hint of an empty outline).
        shown = on_screen(binary, env, p, FIRST_ITEM_ROW, INK)
        # The output is 800 by 500 in its own units at a scale of 2.
        steps = [typed_in, shown, told(hand, f"move {FIRST_ITEM[0]} {FIRST_ITEM[1]} 800 500", "click"),
                 told(keys, "type Z", f"key {EV_F10}")]
        status, err = ended(p)
        es = entries(j)
        first = items(es)[0] if items(es) else None
        into = [(e["body"].get("item"), e["body"].get("add")) for e in es
                if e["kind"] == "edit" and e["body"].get("op") == "text"]
        check("under Wayland at a scale of 2, a click on the first item puts the next key there",
              keyboard_ready and pointer_ready and focused and all(steps) and status == 0
              and (first, "Z") in into, (keyboard_ready, pointer_ready, focused, steps, status, first, into, err[-200:]))
    finally:
        finished(keys)
        finished(hand)
        sway.kill()


def main():
    binary = os.path.abspath(sys.argv[1])
    failures = 0

    def check(what, ok, detail=""):
        nonlocal failures
        print(("ok:   " if ok else "FAIL: ") + what + ("" if ok else f"\n      {detail}"))
        failures += 0 if ok else 1

    with tempfile.TemporaryDirectory() as d:
        # The user's own configuration must not change what is tested.
        os.environ.pop("ADVERSARIA_CONFIG", None)
        os.environ["XDG_CONFIG_HOME"] = os.path.join(d, "no-config")
        # A wrong configuration: refused before any window is opened.
        conf = os.path.join(d, "wrong.toml")
        with open(conf, "w", encoding="utf-8") as f:
            f.write("[quasimodes]\nfrobnicate = \"F5\"\n")
        j = os.path.join(d, "refused")
        r = subprocess.run([binary, "--config", conf, j], capture_output=True, text=True,
                           env=dict(os.environ, DISPLAY="", WAYLAND_DISPLAY=""))
        check("a wrong configuration is refused", r.returncode == 2, f"status {r.returncode}")
        check("the refusal names the file and the fault", conf in r.stderr and "frobnicate" in r.stderr, r.stderr)
        check("nothing is written when the configuration is refused", not os.path.exists(j))
        x11_checks(binary, d, check)
        wayland_checks(binary, d, check)
        wayland_click(binary, d, check)

    print(f"{failures} failed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
