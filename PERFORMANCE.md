# Performance

This is a benchmark of Blue Dove against other common music players built on GTK3, GTK4 and Qt6: DeaDBeeF (GTK3), g4music (GTK4) and Strawberry (Qt6). The numbers come from playing one track of a real album. They are indicative rather than a benchmark suite: one run of each player, on one machine.

## Test material

- Album: `Ace Combat 8 - Wings of Theve`, 76 tracks, Ogg Vorbis, 48 kHz stereo.
- Track: `03 - Debriefing 1`, 2:24.
- Each player loaded the whole album, selected that track, and played it.
- The sample is 10 seconds, taken after 10 seconds of playback.

## Environment

- Artix Linux, kernel 7.2.14, X11 with i3.
- Intel Xeon E5-1650 v4 (6 cores, 12 threads), 16 GiB RAM.
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
| g4music | 4.6.2 | GTK4 | GTK4 - GL |
| g4music | 4.6.2 | GTK4 | GTK4 - Cairo |

## Results

### Winner by CPU usage

| # | Player | Renderer | CPU (one core) |
|--:|---|---|---:|
| 1 | DeaDBeeF | GTK3 | 2.29 % |
| 2 | **Blue Dove** | OpenGL | 4.87 % |
| 3 | **Blue Dove** | software | 6.97 % |
| 4 | Strawberry | Qt6 | 7.82 % |
| 5 | g4music | GTK4 - GL | 8.67 % |
| 6 | g4music | GTK4 - Cairo | **72.84 %** (lol?) |

### Winner by RSS

| # | Player | Renderer | RSS |
|--:|---|---|---:|
| 1 | **Blue Dove** | software | 49.3 MB |
| 2 | DeaDBeeF | GTK3 | 103.7 MB |
| 3 | **Blue Dove** | OpenGL | 107.3 MB |
| 4 | g4music | GTK4 - Cairo | 158.7 MB |
| 5 | Strawberry | Qt6 | 166.6 MB |
| 6 | g4music | GTK4 - GL | 185.9 MB |

### Winner by PSS

| # | Player | Renderer | PSS |
|--:|---|---|---:|
| 1 | **Blue Dove** | software | 42.6 MB |
| 2 | DeaDBeeF | GTK3 | 57.8 MB |
| 3 | **Blue Dove** | OpenGL | 63.3 MB |
| 4 | g4music | GTK4 - Cairo | 107.0 MB |
| 5 | g4music | GTK4 - GL | 126.6 MB |
| 6 | Strawberry | Qt6 | 138.1 MB |

### Combined

| Overall | Player | Renderer | CPU | RSS | PSS |
|--:|---|---|---:|---:|---:|
| 1 | DeaDBeeF | GTK3 | 2.29 % | 103.7 MB | 57.8 MB |
| 2 | **Blue Dove** | software | 6.97 % | 49.3 MB | 42.6 MB |
| 3 | **Blue Dove** | OpenGL | 4.87 % | 107.3 MB | 63.3 MB |
| 4 | g4music | GTK4 - Cairo | 72.84 % | 158.7 MB | 107.0 MB |
| 5 | Strawberry | Qt6 | 7.82 % | 166.6 MB | 138.1 MB |
| 6 | g4music | GTK4 - GL | 8.67 % | 185.9 MB | 126.6 MB |

- DeaDBeeF wins overall on the combined CPU, RSS and PSS results.
- CPU is the whole player process tree, normalized to one core: 100 % is one core, not the whole machine.
- RSS is the sum of the process tree. PSS splits shared pages between the processes that map them.
- The Cairo result shows that software rasterization of the full GTK scene is not viable at this window size.

## Notes

- The album is decoded with Ogg Vorbis, and the players also hold the 76-track album and its cover, which is part of the cost above.
- The numbers are from a single 10-second sample. Treat small differences as noise.

# Improving current performance

DeaDBeeF is the interesting result: it is software rendered (GTK3/Cairo) and still uses the least CPU. That is not because software rendering is cheap. It is because GTK only repaints what changed. Its seekbar timer runs at `gtkui.refresh_rate` (6 Hz here), and calls `gtk_widget_queue_draw` on the seekbar widget alone. Cairo redraws that small damaged strip; the playlist, album list, cover and menus are left as they were.

Blue Dove is built on egui, which is immediate mode. A frame lays out and tessellates the whole window, and the renderer paints the whole framebuffer, whether one pixel of the seek cursor changed or not. egui and eframe do not expose a damage region, so there is no cheap "repaint the waveform strip only" path. The cursor is the only continuously moving element, so its repaint rate is derived from the track length and the bar width: about eight frames a second on this track, one point of movement per frame. Going lower makes the cursor visibly step; going higher multiplies the cost of a full-window frame. g4music with Cairo shows the other extreme: rasterizing a full GTK4 scene in software costs 72.84 %, so DeaDBeeF's number is not evidence that software rendering is cheap.

That is the gap to close. Matching DeaDBeeF's CPU would need partial repaint: cache the static window as a texture and draw only the seekbar into it, move the seekbar to an overlay surface, or add dirty rectangles to the renderer. Those are architectural changes rather than tuning, and they become platform specific quickly. The current work (one waveform mesh, repaint on cursor movement, opaque software drawing and fewer waveform columns) removes the obvious waste; the remaining difference is the full-window redraw egui performs every frame.
