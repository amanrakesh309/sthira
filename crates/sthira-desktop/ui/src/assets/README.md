# Ambient artwork

The bundled artwork follows `docs/design/references/home.png`. It is decorative, local, and independent of vault content. The original concept image supplied style guidance; no UI or example text was copied into these assets.

- `landscape-watercolor.webp`: sage/stone watercolor mountain panorama, used in headers and the Connections rail. Project cards use an ochre color filter.
- `olive-watercolor.webp`: five-leaf watercolor olive twig, used on project cards and in Settings navigation.
- `paper-grain.svg`: small, static, repeating paper grain.
- `terrain-contours.svg`: locally authored decorative contour lines, faded into the workspace corners. These are abstract terrain shapes, not geographical data. The companion sage/clay washes are CSS gradients with semantic theme tokens; document sheets and data surfaces remain opaque.

The raster assets were created with the built-in imagegen tool with transparent backgrounds, then encoded as WebP at quality 92 while preserving alpha. Runtime CSS blends the pigment into warm paper in light mode and uses a separately tuned filter and screen blend in dark mode. Artwork is noninteractive, yields on constrained windows, and follows rather than overlaps reference content.

## Generation prompts

### Landscape

Create a production UI background artwork asset. Use the attached Sthira concept screenshot ONLY as the style reference for its softly painted mountains in the top-right header; do not reproduce any UI, text, panels or screenshots. A wide panoramic arrangement of 4 irregular natural mountain ridgelines, overlapping atmospheric hills in muted olive sage, soft grey-green, warm pale stone. Authentic delicate watercolor on cold-press paper, granulated pigment, dry-brush broken edges, variations in pigment pooling, organic detailed silhouette, not smooth sinusoidal waves or rounded cartoon hills. Sparse mountains across the bottom two thirds; top third and all four outer edges fade gradually into true transparency, especially bottom and left. Distant pale beige hills behind a more defined sage green ridge. Quiet luminous painterly look, subtle but visible green pigment, plenty of airy negative space. Wide landscape composition about 3:1. No sky block or white rectangle, no frame, no sun, no trees, no typography, no lettering, no icons. Transparent background.

### Olive twig

Create a production UI illustration asset. Use the attached Sthira concept screenshot ONLY as style inspiration for the small olive leaves on its first project card; do not reproduce UI, screenshot, text, cards or mountains. One elegant slender olive twig rising diagonally from lower left to upper right, with five narrow graceful pointed leaves, muted sage and olive grey greens. Authentic delicate watercolor botanical study: textured granulated pigment, uneven transparent color, delicate darker central veins and thin stems, softly broken dry-brush edges. Loose luminous natural brushwork, understated elegant warm editorial aesthetic. Large enough to see the painterly texture. Twig and leaves entirely within the canvas with generous transparent margins. No pot, no flowers, no lettering, no label, no shadows, no background wash. Genuine transparent background.
