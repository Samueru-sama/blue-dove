# Performance

This is a benchmark of Blue Dove against other common music players built on GTK3, GTK4 and Qt6: DeaDBeeF (GTK3), g4music (GTK4) and Strawberry (Qt6). The numbers come from playing one track of a real album. They are indicative rather than a benchmark suite: one run of each player, on one machine.

## Test material

- Album: `Ace Combat 8 - Wings of Theve`, 76 tracks, Ogg Vorbis, 48 kHz stereo.
- Track: `03 - Debriefing 1`, 2:24.
- Each player loaded the whole album, selected that track, and played it.
- The sample is 10 seconds, taken after 10 seconds of playback.

## Environment

- Artix Linux, kernel 7.2.14, X11 with i3.
- Intel Xeon E5-1650 v4 (6 cores, 12 threads), 15 GiB RAM.
- AMD Radeon RX 580, Mesa 26.2.6.
- PulseAudio output.
- One 1200x800 window, one player running at a time, a fresh profile for each run.

## Players

| Player | Version | Toolkit | Renderer |
|---|---|---|---|
| **Blue Dove** | 0.1.16 | egui | OpenGL |
| **Blue Dove** | 0.1.16 | egui | software |
| DeaDBeeF | 1.10.3 | GTK3 | default |
| Strawberry | 1.2.31 | Qt6 | default |
| g4music | 4.6.2 | GTK4 | GSK, GL |
| g4music | 4.6.2 | GTK4 | GSK, Cairo |

## Results

| Player | Renderer | CPU (one core) | RSS | PSS |
|---|---|---:|---:|---:|
| DeaDBeeF | GTK3 | 2.29 % | 103.7 MB | 57.8 MB |
| **Blue Dove** | OpenGL | 4.87 % | 107.3 MB | 63.3 MB |
| **Blue Dove** | software | 6.97 % | 49.3 MB | 42.6 MB |
| Strawberry | Qt6 | 7.82 % | 166.6 MB | 138.1 MB |
| g4music | GL | 8.67 % | 185.9 MB | 126.6 MB |
| g4music | Cairo | 72.84 % | 158.7 MB | 107.0 MB |

- CPU is the whole player process tree, normalized to one core: 100 % is one core, not the whole machine.
- RSS is the sum of the process tree. PSS splits shared pages between the processes that map them.
- The Cairo result shows that software rasterization of the full GTK scene is not viable at this window size.

## Notes

- The album is decoded with Ogg Vorbis, and the players also hold the 76-track album and its cover, which is part of the cost above.
- The numbers are from a single 10-second sample. Treat small differences as noise.
