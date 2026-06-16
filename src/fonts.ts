// Bundles the Involve typeface (shipped in /Assets/FONTS) into the document.
// Importing the .ttf yields a Vite-resolved URL that works in dev and build.
import regular from "../Assets/FONTS/Involve-Regular.ttf";
import bold from "../Assets/FONTS/Involve-Bold.ttf";

export function injectFonts(): void {
  const style = document.createElement("style");
  style.textContent = `
    @font-face {
      font-family: 'Involve';
      src: url('${regular}') format('truetype');
      font-weight: 400; font-style: normal; font-display: swap;
    }
    @font-face {
      font-family: 'Involve';
      src: url('${bold}') format('truetype');
      font-weight: 700; font-style: normal; font-display: swap;
    }`;
  document.head.appendChild(style);
}
