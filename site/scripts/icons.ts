// Renders the page's icon images from the app icon. Run after the icon changes:
//   bun scripts/icons.ts
import sharp from "sharp";

const source = new URL("../../assets/icon/AppIcon-1024.png", import.meta.url).pathname;
const media = new URL("../src/media/", import.meta.url).pathname;

for (const size of [64, 192, 256]) {
  await sharp(source).resize(size, size).webp({ quality: 90 }).toFile(`${media}icon-${size}.webp`);
}
await sharp(source).resize(32, 32).png().toFile(`${media}favicon-32.png`);
await sharp(source).resize(180, 180).flatten({ background: "#000000" }).png().toFile(`${media}apple-touch-icon.png`);
console.log("icons written to", media);
