# Génère guitar_dry_demo.wav : 10 s de guitare synthétique (Karplus-Strong),
# accords Em / C / G — source sèche pour tester PedalReverb dans REAPER.
import wave, math, struct, random

SR = 44100

def pluck(freq, dur, amp=0.5, damp=0.996):
    n = int(SR * dur)
    period = max(2, int(SR / freq))
    random.seed(int(freq * 1000))
    buf = [random.uniform(-1, 1) for _ in range(period)]
    out = []
    for i in range(n):
        out.append(buf[i % period])
        buf[i % period] = damp * 0.5 * (buf[i % period] + buf[(i + 1) % period])
    return [sample * amp for sample in out]

def chord(freqs, dur, amp, offset=0.0):
    tracks = [pluck(f, dur, amp / len(freqs)) for f in freqs]
    n = int(SR * dur)
    out = [0.0] * n
    for t in tracks:
        for i, s in enumerate(t):
            out[i] += s
    return out

def place(dest, src, at_sec):
    at = int(at_sec * SR)
    for i, s in enumerate(src):
        j = at + i
        if j < len(dest):
            dest[j] += s

# Accords : Em (E2 B2 E3 G3 B3 E4), C (C3 E3 G3 C4 E4), G (G2 D3 G3 B3 D4)
E_midi = 82.41, 123.47, 164.81, 196.00, 246.94, 329.63
C_ch = 130.81, 164.81, 196.00, 261.63, 329.63
G_ch = 98.00, 146.83, 196.00, 246.94, 293.66

total = 10.0
mix = [0.0] * int(SR * total)

SEQ = [
    (0.0,  E_midi, 3.0),
    (3.0,  C_ch,   3.0),
    (6.0,  G_ch,   3.5),
    (6.5,  E_midi, 3.0),   # léger chevauchement pour nourrir la reverb
]
for at, freqs, dur in SEQ:
    place(mix, chord(freqs, dur, 0.8), at)

# Normalisation douce
peak = max(abs(s) for s in mix) or 1.0
if peak > 0.95:
    k = 0.95 / peak
    mix = [s * k for s in mix]

with wave.open("guitar_dry_demo.wav", "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, s)) * 32767)) for s in mix))
print("guitar_dry_demo.wav écrit :", total, "s mono", SR, "Hz")
