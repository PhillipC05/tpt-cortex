// Package tray renders a system tray icon for the Cortex daemon.
// Run() must be called on the main OS thread (main goroutine).
package tray

import (
	"github.com/getlantern/systray"
)

// Run starts the tray icon and blocks until the user clicks Quit.
// tooltip is shown on hover. onQuit is called just before the process exits.
func Run(tooltip string, onQuit func()) {
	systray.Run(func() {
		systray.SetIcon(makeCortexIcon())
		systray.SetTitle("Cortex")
		systray.SetTooltip(tooltip)

		mStatus := systray.AddMenuItem("Cortex Daemon — Running", "")
		mStatus.Disable()
		systray.AddSeparator()
		mQuit := systray.AddMenuItem("Quit", "Stop the Cortex daemon")

		go func() {
			for range mQuit.ClickedCh {
				systray.Quit()
			}
		}()
	}, func() {
		if onQuit != nil {
			onQuit()
		}
	})
}

// makeCortexIcon generates a minimal 16×16 32bpp ICO file in memory.
// Solid #4AB56E (Cortex green). No external file dependency.
func makeCortexIcon() []byte {
	const w, h = 16, 16

	// XOR map — bottom-up BGRA pixels, solid Cortex green.
	pixels := make([]byte, w*h*4)
	for i := 0; i < w*h; i++ {
		pixels[i*4+0] = 0x6E // B
		pixels[i*4+1] = 0xB5 // G
		pixels[i*4+2] = 0x4A // R
		pixels[i*4+3] = 0xFF // A (fully opaque)
	}

	// AND mask — all zeros (fully visible).
	// Row width: ceil(16/8)=2 bytes, padded to 4-byte alignment → 4 bytes/row.
	andMask := make([]byte, h*4)

	bmpSize := 40 + len(pixels) + len(andMask)

	buf := make([]byte, 0, 22+bmpSize)

	// ICONDIR (6 bytes)
	buf = append(buf, 0x00, 0x00) // reserved
	buf = append(buf, 0x01, 0x00) // type = ICO
	buf = append(buf, 0x01, 0x00) // image count = 1

	// ICONDIRENTRY (16 bytes)
	buf = append(buf, byte(w), byte(h), 0x00, 0x00) // width, height, colorCount, reserved
	buf = append(buf, 0x01, 0x00)                   // biPlanes
	buf = append(buf, 0x20, 0x00)                   // biBitCount = 32
	buf = appendU32(buf, uint32(bmpSize))            // bytesInRes
	buf = appendU32(buf, 22)                         // imageOffset = 6+16

	// BITMAPINFOHEADER (40 bytes)
	buf = appendU32(buf, 40)                       // biSize
	buf = appendU32(buf, w)                        // biWidth
	buf = appendU32(buf, h*2)                      // biHeight = 2×h (XOR+AND stacked)
	buf = append(buf, 0x01, 0x00)                  // biPlanes
	buf = append(buf, 0x20, 0x00)                  // biBitCount = 32
	buf = appendU32(buf, 0)                        // biCompression = BI_RGB
	buf = appendU32(buf, uint32(len(pixels)))      // biSizeImage
	buf = appendU32(buf, 0)                        // biXPelsPerMeter
	buf = appendU32(buf, 0)                        // biYPelsPerMeter
	buf = appendU32(buf, 0)                        // biClrUsed
	buf = appendU32(buf, 0)                        // biClrImportant

	buf = append(buf, pixels...)
	buf = append(buf, andMask...)
	return buf
}

func appendU32(b []byte, v uint32) []byte {
	return append(b, byte(v), byte(v>>8), byte(v>>16), byte(v>>24))
}
