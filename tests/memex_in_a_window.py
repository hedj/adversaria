#!/usr/bin/env python3
"""Run the memex program in its window, with this script typing into it.

Under X11 it starts an X server with no screen (Xvfb) and types with
xdotool by key code, since xdotool's key names press Left Alt with Right
Alt, and wraps later keys in Alt. Under Wayland it starts a headless sway
and types with wtype, whose virtual keyboard makes up its own key codes, so
only text and named keys are typed there, not quasimodes held on modifiers.
It checks what the program does: the journal entries the keys should make,
and its exit status.

Usage: tests/memex_in_a_window.py <path to the memex binary>
Needs: Xvfb, xdotool, sway and wtype (the Nix shell has them), and the
window's libraries and drivers (the Nix shell sets them).
"""
import json
import os
import shutil
import signal
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


def wait_for_window(env, timeout=30, process=None):
    """The memex window's id, once it is mapped; none if it is not, or if
    the program ends first."""
    end = time.time() + timeout
    while time.time() < end and (process is None or process.poll() is None):
        r = subprocess.run(["xdotool", "search", "--name", "^memex$"], env=env,
                           capture_output=True, text=True)
        if r.stdout.strip():
            return r.stdout.split()[0]
        time.sleep(0.3)
    return None


def run_x11(binary, display, journal, actions, args=(), kill_after=None, timeout=20):
    """Run the memex on a journal, type the actions into its window, and
    return its exit status and what it wrote to stderr."""
    env = dict(os.environ, DISPLAY=display)
    env.pop("WAYLAND_DISPLAY", None)
    p = subprocess.Popen([binary, *args, journal], env=env,
                         stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    w = wait_for_window(env, process=p)
    if w is None:
        if p.poll() is None:
            p.kill()
        _, err = p.communicate()
        return (p.returncode if p.returncode >= 0 else None), err.decode("utf-8", "replace")
    subprocess.run(["xdotool", "windowfocus", "--sync", w], env=env)
    for a in actions:
        subprocess.run(["xdotool", *a], env=env)
    if kill_after is not None:
        time.sleep(kill_after)
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
    xvfb = subprocess.Popen(["Xvfb", ":91", "-screen", "0", "1400x900x24"],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.5)
    try:
        x11_typing(binary, d, check)
        x11_remapped(binary, d, check)
        x11_mouse(binary, d, check)
        x11_idle_and_reasoning(binary, d, check)
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
    status, _ = run_x11(binary, ":91", j, [typed("Kept")], kill_after=2.0)
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


def wayland_checks(binary, d, check):
    """Text and named keys under a headless Wayland compositor."""
    run_dir = tempfile.mkdtemp(dir=d)
    os.chmod(run_dir, 0o700)
    cfg = os.path.join(run_dir, "sway.cfg")
    with open(cfg, "w", encoding="utf-8") as f:
        f.write("output HEADLESS-1 resolution 1280x800\nxwayland disable\n")
    env = dict(os.environ, XDG_RUNTIME_DIR=run_dir, WLR_BACKENDS="headless",
               WLR_LIBINPUT_NO_DEVICES="1", WLR_RENDERER="pixman")
    env.pop("DISPLAY", None)
    sway = subprocess.Popen(["sway", "-c", cfg], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        sockets = []
        for _ in range(50):
            sockets = [s for s in os.listdir(run_dir) if s.startswith("wayland-") and not s.endswith(".lock")]
            if sockets:
                break
            time.sleep(0.2)
        env["WAYLAND_DISPLAY"] = sockets[0] if sockets else ""
        j = os.path.join(d, "wayland")
        p = subprocess.Popen([binary, j], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        time.sleep(6)
        # One wtype run, its first key a Shift: each run makes a new virtual
        # keyboard, and the first key after it is lost (seen under sway;
        # a real keyboard's map is sent once, on focus, untested here).
        keys = ["-d", "50", "-k", "Shift_L", "Hello", "-k", "Return", "World", "-k", "F10"]
        subprocess.run(["wtype", *keys], env=env)
        try:
            _, err = p.communicate(timeout=20)
            status = p.returncode
        except subprocess.TimeoutExpired:
            p.kill()
            status, err = None, b"timed out"
        check("under Wayland it opens a window and quits with F10", status == 0,
              f"status {status}: {err.decode('utf-8', 'replace')[-300:]}")
        check("under Wayland typed text reached the journal",
              "Hello" in texts(entries(j)) and "World" in texts(entries(j)), texts(entries(j)))
    finally:
        sway.kill()
        shutil.rmtree(run_dir, ignore_errors=True)


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

    print(f"{failures} failed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
