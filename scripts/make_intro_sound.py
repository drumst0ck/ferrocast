#!/usr/bin/env python3
"""Synthesize the first-launch intro sound (resources/intro.m4a).

Timed to the intro animation in crates/app/src/intro.rs: a warm swell while the red glow rises
(0-2.6 s), a soft airy lift as the logo appears (0.9-1.9 s), one small bell per letter of
FERROCAST (from 1.8 s, 60 ms apart), and a bloom that fades with the hand-off (3.4-3.9 s).

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


# 1. Swell: an A major add9 chord, slightly detuned pairs, brightening as it rises.
chord = [110.0, 164.81, 220.0, 277.18, 329.63, 493.88]
rise = smooth(t / 2.6)
fade = 1.0 - smooth((t - 3.3) / 1.1)
for i, f in enumerate(chord):
    for detune in (-0.6, 0.6):
        ph = 2 * np.pi * (f + detune) * t
        tone = np.sin(ph) + rise * 0.35 * np.sin(2 * ph) + rise**2 * 0.12 * np.sin(3 * ph)
        out += tone * (0.055 if i < 2 else 0.035)
out *= rise**1.4 * fade

# 2. Lift: band-passed noise sweeping up while the logo appears.
noise = rng.standard_normal(len(t))
spec = np.fft.rfft(noise)
freqs = np.fft.rfftfreq(len(t), 1 / SR)
air = np.zeros_like(t)
for centre in np.geomspace(400, 5000, 12):
    band = np.fft.irfft(spec * np.exp(-((np.log(freqs + 1) - np.log(centre)) ** 2) / 0.08), len(t))
    when = 0.9 + 1.0 * (np.log(centre / 400) / np.log(5000 / 400))
    air += band * np.exp(-((t - when) ** 2) / 0.05)
out += 0.018 * air / (np.abs(air).max() + 1e-9) * 6

# 3. Bells: one per letter, rising through the A major pentatonic.
notes = [880.0, 987.77, 1108.73, 1318.51, 1479.98, 1760.0, 1975.53, 2217.46, 2637.02]
for n in range(LETTERS):
    at = WORD_AT + n * LETTER_STEP
    k = t >= at
    dt = t[k] - at
    f = notes[n]
    bell = np.sin(2 * np.pi * f * dt) + 0.3 * np.sin(2 * np.pi * f * 2.76 * dt) * np.exp(-dt * 9)
    out[k] += 0.05 * bell * np.exp(-dt * 3.2) * (1 - np.exp(-dt * 400))

# 4. Bloom: the chord an octave up, at the moment the word is complete.
at = WORD_AT + LETTERS * LETTER_STEP
k = t >= at
dt = t[k] - at
for f in (440.0, 554.37, 659.26, 880.0):
    out[k] += 0.03 * np.sin(2 * np.pi * f * dt) * (1 - np.exp(-dt * 12)) * np.exp(-dt * 1.6)

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
