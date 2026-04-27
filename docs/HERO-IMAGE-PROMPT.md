# Hero Image Prompt - lithium

Generation target: a single banner image at the top of the GitHub README. Aspect ratio 2:1, output at 1600x800 minimum. Will sit above the project title in the rendered README.

Two prompts below: a primary, and a backup. Run the primary first; if the result is over-literal or too "AI-glossy," run the backup.

## Primary Prompt

```
Cinematic hero banner, 1600x800 pixels, designed for the top of a GitHub README for an open-source developer tool.

Subject: A stylized periodic-table tile reading "3 / Li / Lithium / 6.94" floating in the center-left, rendered as a glowing translucent crystalline cube. Behind it, a vintage analog voltmeter / pressure gauge with a needle pinned in the green safe zone, labeled in tiny technical typography "API SPEND - STABLE." Beneath both, a faint outline of three subtle provider logos (Anthropic, OpenAI, OpenRouter) rendered as flat geometric icons in the negative space, like watermarks on technical drafting paper. A thin power-cable-style line connects the lithium tile down to the gauge needle, suggesting the element regulates the reading.

Mood: a chemistry lab meets a mid-century instrument panel. Quietly powerful. The visual joke: lithium-the-element AND lithium-the-mood-stabilizer, both keeping a runaway system steady. The viewer should feel that whatever this tool measures, it is currently and reliably under control.

Color palette: deep navy (#0A1628) and warm matte black (#171717) background, electric cyan (#00E5FF) for the lithium glow, warm amber (#FFA000) on the gauge face, sharp clean white (#FFFFFF) for typography, with a single accent of lithium-pink (#E86996) on the gauge needle. Subtle teal grid pattern in the negative space, like graph paper.

Style: cinematic concept art with a faint film grain. Slightly painterly, NOT photorealistic. Think the visual sensibility of Mr. Robot title cards crossed with vintage Bell Labs technical posters. Crisp, readable typography on the periodic-table tile and gauge labels. Hand-crafted by-a-senior-designer feel, NOT generic AI-generated polish.

Composition: lithium tile and gauge dominate the upper-left two-thirds. The right third is intentional negative space (for the project title to be overlaid in HTML; do NOT include the word "lithium" in the image itself). Provider-logo watermarks fade into the lower edges.

Strict constraints: no people, no faces, no hands, no realistic Apple devices, no terminal screenshots, no chat bubbles, no AI-glossy gradients, no neon-everything. The aesthetic is restrained technical authority, not a startup landing page.
```

## Backup Prompt (if primary lands too literal or too cluttered)

```
Minimalist hero banner, 1600x800 pixels, for an open-source developer tool's GitHub README.

Subject: A single oversized periodic-table tile reading "3 / Li / Lithium / 6.94" rendered as a translucent glass cube in the right-of-center position. The cube has a soft inner glow, like phosphorescent mineral. To its left, on a clean dark background, a barely-visible technical schematic of a flow meter or check-valve in white-line-on-black, hinting at "regulating something."

Mood: confident understatement. The kind of cover image a serious infrastructure tool would have. The viewer should feel the project takes itself seriously without trying too hard.

Color palette: matte black background (#0A0A0A), translucent lithium-cyan (#00E5FF) for the cube, white-line schematic (#FFFFFF at 30% opacity), one accent of lithium-pink (#E86996) for the smallest detail.

Style: vector-clean, concept-art-restrained, slight film grain. NOT photorealistic. Think Linear's marketing pages or Tailwind's hero sections, but with a periodic-table-element instead of an abstract gradient blob.

Composition: heavy negative space on the left two-thirds (for project title to overlay in HTML). Cube anchors the right.

Strict constraints: no people, no devices, no terminals, no AI-glossy gradients, no over-decoration, no the word "lithium" in the image itself, no neon overload. Pure infrastructure-grade restraint.
```

## Tips for the operator (you)

- ChatGPT image gen will sometimes ignore the "no text" constraint. If "lithium" appears in the image, regenerate.
- If the result feels generic / floaty / aesthetic-mood-board, it's the AI-glossy trap. Push back: "more restrained, less neon, more technical-poster, less marketing."
- Aspect ratio matters. 2:1 is what GitHub READMEs render at full width. Don't accept square crops.
- Budget 3-5 generations to get one keeper. Save the others to `assets/hero-alts/` in case Phase 2 wants a refresh.
- Once you have the keeper, run it through a slight downsample to soften any AI artifacts at full resolution.

## After generation

1. Save the chosen image to `assets/hero.png` (PNG preferred over JPG for the typography sharpness).
2. Verify it renders correctly in the README locally:
   ```
   cd ~/projects/lithium
   open README.md   # or use a markdown previewer
   ```
3. If the image is too tall, crop to 2:1 in Preview or `sips`.
4. Commit it. The README is already wired to expect `assets/hero.png`.
