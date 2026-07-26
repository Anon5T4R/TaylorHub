import { describe, expect, it } from "vitest";

import { globMatch, installFormat } from "../hub";

/**
 * Estes testes existem porque `installFormat` é uma SEGUNDA implementação de
 * uma regra que já vive no Rust (`pkg::asset_globs_in_order`). Duas cópias da
 * mesma regra divergem — a questão é quando. Aqui a divergência vira teste
 * vermelho em vez de o Hub prometer "vai instalar pacote pacman" e instalar um
 * AppImage.
 */

const assetsCompleto = [
  { name: "TaylorHub_0.24.2_amd64.AppImage" },
  { name: "TaylorHub_0.24.2_amd64.deb" },
  { name: "taylor-hub-0.24.2-1-x86_64.pkg.tar.zst" },
  { name: "TaylorHub_0.24.2_x64-setup.exe" },
];
const soAppImage = [{ name: "LocalCode_0.8.7_amd64.AppImage" }];

describe("globMatch", () => {
  it("casa curinga no meio e no fim", () => {
    expect(globMatch("*_amd64.AppImage", "App_1.2.3_amd64.AppImage")).toBe(true);
    expect(globMatch("*.pkg.tar.zst", "taylor-hub-1-x86_64.pkg.tar.zst")).toBe(true);
  });

  it("ignora maiuscula/minuscula, como o Rust", () => {
    expect(globMatch("*.appimage", "App_amd64.AppImage")).toBe(true);
  });

  it("sem `*` no fim, exige terminar onde o padrao parou", () => {
    // `.deb` nao pode casar um `.deb.sig` — senao o Hub anunciaria um formato
    // olhando pra um arquivo de assinatura.
    expect(globMatch("*_amd64.deb", "App_1.0_amd64.deb.sig")).toBe(false);
    expect(globMatch("*_amd64.deb", "App_1.0_amd64.deb")).toBe(true);
  });

  it("nao casa o que nao existe", () => {
    expect(globMatch("*.pkg.tar.zst", "App_amd64.AppImage")).toBe(false);
  });
});

describe("installFormat", () => {
  it("no Arch prefere o pacote nativo", () => {
    expect(installFormat(assetsCompleto, "linux", "pacman")).toBe("pacman");
  });

  it("no Debian prefere o .deb", () => {
    expect(installFormat(assetsCompleto, "linux", "apt")).toBe("deb");
  });

  it("sem gerenciador conhecido, AppImage", () => {
    expect(installFormat(assetsCompleto, "linux", "")).toBe("appimage");
  });

  it("CAI no AppImage quando a release ainda nao tem pacote nativo", () => {
    // O caso do rollout: o pacote nativo entrou app por app. Anunciar "pacman"
    // numa release que so tem AppImage seria mentir para o usuario.
    expect(installFormat(soAppImage, "linux", "pacman")).toBe("appimage");
    expect(installFormat(soAppImage, "linux", "apt")).toBe("appimage");
  });

  it("no Windows, o instalador", () => {
    expect(installFormat(assetsCompleto, "windows", "")).toBe("exe");
  });

  it("release sem nenhum asset servivel nao inventa formato", () => {
    expect(installFormat([{ name: "leiame.txt" }], "linux", "pacman")).toBe("desconhecido");
    expect(installFormat([], "windows", "")).toBe("desconhecido");
  });

  it("respeita o fallback do catalogo para apps fora do padrao de nome", () => {
    // LocalMind/OpenObsidian publicam `Nome-1.2.3.AppImage`, sem `_amd64`.
    const fora = [{ name: "OpenObsidian-1.2.3.AppImage" }];
    expect(installFormat(fora, "linux", "pacman", "OpenObsidian*.AppImage")).toBe("appimage");
  });
});
