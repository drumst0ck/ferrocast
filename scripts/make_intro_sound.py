#!/usr/bin/env python3
"""Synthesize the first-launch intro sound (resources/intro.m4a).

Timed to the intro animation in crates/app/src/intro.rs: a dark, low swell while the red glow
rises (0-2.6 s), a low breath as the logo appears (0.9-1.9 s), and a deep bloom when FERROCAST is
complete that fades with the hand-off (3.4-3.9 s). Nothing bright: everything above ~1.8 kHz is
rolled off.

    scripts/make_intro_sound.py            # needs numpy; afconvert ships with macOS
"""
import pathlib
import subprocess
import tempfile
import wave

import numpy as np

SR = 44100
LENGTH = 4.6  # the reverb tail rings a little past the 3.9 s animation
WORD_AT, LETTER_STEP, LETTERS = 1.8, 0.06, 9
rng = np.random.default_rng(7)
t = np.arange(int(SR * LENGTH)) / SR
out = np.zeros_like(t)


def smooth(x):
    x = np.clip(x, 0.0, 1.0)
    return x * x * (3 - 2 * x)


# 1. Swell: a low A minor chord (with a sub octave), slightly detuned pairs.
chord = [55.0, 82.41, 110.0, 130.81, 164.81]
rise = smooth(t / 2.6)
fade = 1.0 - smooth((t - 3.3) / 1.1)
for i, f in enumerate(chord):
    for detune in (-0.6, 0.6):
        ph = 2 * np.pi * (f + detune) * t
        tone = np.sin(ph) + rise * 0.22 * np.sin(2 * ph)
        out += tone * (0.07 if i < 2 else 0.04)
out *= rise**1.4 * fade

# 2. Breath: low band-passed noise rising a little while the logo appears.
noise = rng.standard_normal(len(t))
spec = np.fft.rfft(noise)
freqs = np.fft.rfftfreq(len(t), 1 / SR)
air = np.zeros_like(t)
for centre in np.geomspace(120, 700, 12):
    band = np.fft.irfft(spec * np.exp(-((np.log(freqs + 1) - np.log(centre)) ** 2) / 0.08), len(t))
    when = 0.9 + 1.0 * (np.log(centre / 120) / np.log(700 / 120))
    air += band * np.exp(-((t - when) ** 2) / 0.05)
out += 0.012 * air / (np.abs(air).max() + 1e-9) * 6

# 3. Bloom: a deep hit when the word is complete; the low chord swells once more and decays.
at = WORD_AT + LETTERS * LETTER_STEP
k = t >= at
dt = t[k] - at
for f, gain in ((41.2, 0.09), (55.0, 0.08), (82.41, 0.05), (110.0, 0.035), (130.81, 0.025)):
    out[k] += gain * np.sin(2 * np.pi * f * dt) * (1 - np.exp(-dt * 25)) * np.exp(-dt * 1.3)

# Dark: roll off everything above ~1.8 kHz.
spec = np.fft.rfft(out)
spec *= 1 / (1 + (np.fft.rfftfreq(len(out), 1 / SR) / 1800.0) ** 4)
out = np.fft.irfft(spec, len(out))

# Room: convolve with a short decaying noise burst (FFT convolution).
ir_len = int(SR * 1.2)
ir = rng.standard_normal(ir_len) * np.exp(-np.arange(ir_len) / SR * 4.5)
ir[0] = 1.0
size = 1 << int(np.ceil(np.log2(len(out) + ir_len)))
wet = np.fft.irfft(np.fft.rfft(out, size) * np.fft.rfft(ir, size), size)[: len(out)]
mix = 0.75 * out + 0.25 * wet / (np.abs(wet).max() + 1e-9) * np.abs(out).max()

# Fades at both ends, then normalize to -3 dBFS.
edge = int(SR * 0.02)
mix[:edge] *= np.linspace(0, 1, edge)
mix[-int(SR * 0.4):] *= np.linspace(1, 0, int(SR * 0.4))
mix *= 0.707 / np.abs(mix).max()

root = pathlib.Path(__file__).resolve().parent.parent
dest = root / "resources" / "intro.m4a"
with tempfile.TemporaryDirectory() as tmp:
    wav = pathlib.Path(tmp) / "intro.wav"
    with wave.open(str(wav), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes((mix * 32767).astype("<i2").tobytes())
    subprocess.run(["afconvert", "-f", "m4af", "-d", "aac", "-b", "96000", str(wav), str(dest)], check=True)
print(dest, dest.stat().st_size, "bytes")
