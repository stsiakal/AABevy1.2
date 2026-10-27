#!/usr/bin/env python3
"""Launch a hidden 1.2 client, enter the world as Moveb, print the PID."""
import os
import re
import subprocess
import sys
import time

LAUNCHER = r"D:/aaemu-target/veh-golden/live/aaemu-launcher.exe"
DRIVER = r"D:/aaemu-target/veh-golden/live/aaemu-clientdriver.exe"
DLL = r"D:/aaemu-target/veh-golden/aadriver/aadriver.dll"
CLIENT = r"D:/AA1.2/bin32/archeage.exe"
PW = r"C:/Users/stsiakal/AppData/Local/Temp/opencode/moveb-pw.txt"
LOG = r"D:/aaemu-live-veh-golden/logs/launcher-moveb.log"
ENV = dict(os.environ, AADRIVER_DLL=DLL)


def driver(*args):
    p = subprocess.run([DRIVER, *args], capture_output=True, text=True, env=ENV)
    return p.stdout.strip() + p.stderr.strip()


def lua(pid, code):
    return driver("lua", "-t", "ui", "--pid", str(pid), code)


def main():
    with open(LOG, "w") as log:
        subprocess.Popen(
            [LAUNCHER, "--client", CLIENT, "--login", "127.0.0.1:33237", "--user", "moveb",
             "--password-file", PW, "--hidden", "--nosplash", "--multi-client",
             "--driver-dll", DLL],
            stdout=log, stderr=subprocess.STDOUT,
        )
    pid = None
    for _ in range(120):
        time.sleep(2)
        try:
            text = open(LOG, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        m = re.search(r"client started \(pid (\d+)\)", text)
        if m:
            pid = int(m.group(1))
            break
    if pid is None:
        raise SystemExit("no client pid")
    for _ in range(90):
        stage = lua(pid, "return X2LoginCharacter:GetCurrentStage()").strip().splitlines()[-1]
        if stage == "3":
            break
        time.sleep(1)
    lua(pid, "X2World:EnterWorld(1,0)")
    for _ in range(60):
        time.sleep(1)
        out = lua(pid, "return X2LoginCharacter:GetCurrentStage(), X2LoginCharacter:GetNumLoginCharacters()")
        if out.strip().endswith("1"):
            break
    lua(pid, "X2LoginCharacter:SelectCharacter(1)")
    time.sleep(1)
    lua(pid, 'characterSelectWindow.bottomPanel.startButton:OnClick("LeftButton")')
    for _ in range(90):
        time.sleep(1)
        if lua(pid, 'return tostring(X2Unit:UnitName("player"))').strip().splitlines()[-1] == "Moveb":
            print(pid)
            return
    raise SystemExit("character did not enter world")


if __name__ == "__main__":
    main()
