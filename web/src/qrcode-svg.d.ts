declare module "qrcode-svg" {
  interface QRCodeOptions {
    content: string;
    padding?: number;
    width?: number;
    height?: number;
    color?: string;
    background?: string;
    ecl?: "L" | "M" | "Q" | "H";
    join?: boolean;
    xmlDeclaration?: boolean;
    container?: "svg" | "g" | "svg-viewbox" | "none";
    pretty?: boolean;
    swap?: boolean;
  }

  export default class QRCode {
    constructor(options: QRCodeOptions | string);
    svg(): string;
    save(file: string, callback: (err: Error | null) => void): void;
  }
}
