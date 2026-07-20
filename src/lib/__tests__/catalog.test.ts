import { describe, expect, it } from "vitest";
import { CATALOG, compareVersions, extensionRoutes } from "../../catalog";

describe("compareVersions", () => {
  // O caso que motiva a função existir: a comparação de string diz que "0.9.0"
  // é maior que "0.10.0" (porque "9" > "1"). Com isso o Hub esconderia o update
  // de 0.9.0 → 0.10.0 e ainda ofereceria o downgrade inverso.
  it("dez é maior que nove apesar da ordem alfabética", () => {
    expect(compareVersions("0.10.0", "0.9.0")).toBeGreaterThan(0);
    expect(compareVersions("0.9.0", "0.10.0")).toBeLessThan(0);
  });

  it("mesma armadilha no patch: 1.0.10 é mais novo que 1.0.9", () => {
    expect(compareVersions("1.0.10", "1.0.9")).toBeGreaterThan(0);
  });

  // Componente mais significativo decide sozinho. Guarda contra implementação
  // que somasse ou comparasse os componentes na ordem errada: 1.99.99 tem
  // números bem maiores mas continua sendo anterior a 2.0.0.
  it("major decide antes de minor e patch, por maior que seja o resto", () => {
    expect(compareVersions("2.0.0", "1.99.99")).toBeGreaterThan(0);
    expect(compareVersions("1.10.0", "1.9.99")).toBeGreaterThan(0);
  });

  // Tag com menos componentes que a versão instalada (ou vice-versa). Sem o
  // preenchimento com zero, `pa[i] - pb[i]` viraria NaN e o `!== 0` deixaria
  // passar — o Hub reportaria update fantasma de 1.2 para 1.2.0 pra sempre.
  it("faltando componente vale zero, então 1.2 e 1.2.0 são a mesma versão", () => {
    expect(compareVersions("1.2", "1.2.0")).toBe(0);
    expect(compareVersions("1.2.0", "1.2")).toBe(0);
    expect(compareVersions("1.2.1", "1.2")).toBeGreaterThan(0);
  });

  // App instalado pelo .msi antigo grava DisplayVersion com quatro componentes
  // ("0.21.1.0"). Se o quarto componente virasse "mais novo", o card acusaria
  // update indevido a cada refresh.
  it("quarto componente zero não inventa versão nova", () => {
    expect(compareVersions("0.21.1.0", "0.21.1")).toBe(0);
    expect(compareVersions("0.21.1", "0.21.1.0")).toBe(0);
  });

  /**
   * Os dois bugs achados nesta passada. Ambos só alcançáveis por repositório
   * ADICIONADO PELO USUÁRIO (`add_custom_repo`), porque os apps do catálogo
   * embutido sempre publicam `vX.Y.Z` — e ambos falhavam do jeito pior:
   * "nenhum update disponível", em silêncio, pra sempre.
   */
  it("prefixo de rotulo na tag nao e lido como numero", () => {
    // `release-1.2.0` virava [0, 2, 0] — a versão publicada parecia MAIS VELHA
    // que a instalada e o update nunca aparecia.
    expect(compareVersions("release-1.2.0", "1.1.0")).toBeGreaterThan(0);
    expect(compareVersions("v2.0.0", "1.9.9")).toBeGreaterThan(0);
  });

  it("pre_lancamento_vem_antes_da_estavel", () => {
    // `1.0.0-rc1` comparava IGUAL a `1.0.0`: quem estava num rc nunca recebia
    // a estável. Regra do semver.
    expect(compareVersions("1.0.0-rc1", "1.0.0")).toBeLessThan(0);
    expect(compareVersions("1.0.0", "1.0.0-rc1")).toBeGreaterThan(0);
    expect(compareVersions("1.0.0-rc1", "1.0.0-rc2")).toBeLessThan(0);
    expect(compareVersions("1.0.0-rc1", "1.0.0-rc1")).toBe(0);
  });

  it("metadado_de_build_nao_conta_como_pre_lancamento", () => {
    // Pelo semver `+algo` não entra na precedência. Tratá-lo como sufixo faria
    // o Hub oferecer "update" pra exatamente a mesma versão, a cada refresh.
    expect(compareVersions("1.2.0+build5", "1.2.0")).toBe(0);
  });
});

describe("extensionRoutes", () => {
  // Contrato documentado no topo do catalog.ts: pra extensão disputada, o
  // PRIMEIRO app do CATALOG é a rota default. Se o Map perdesse a ordem de
  // inserção (ou a lista fosse ordenada), "md" abriria no LocalCode em vez do
  // LocalWriter sem ninguém ter mudado o catálogo.
  it("extensão disputada preserva a ordem do catálogo", () => {
    const routes = extensionRoutes();
    const md = routes.get("md");
    expect(md?.map((a) => a.id)).toEqual(["writer", "code"]);
  });

  // O App.tsx deriva a extensão do nome do arquivo com slice(dot + 1) +
  // toLowerCase (App.tsx:59). Entrada no catálogo com ponto ou maiúscula nunca
  // casa: a associação fica listada na UI e simplesmente não roteia nada.
  it("nenhuma extensão do catálogo tem ponto ou maiúscula", () => {
    for (const [ext] of extensionRoutes()) {
      expect(ext).toBe(ext.toLowerCase());
      expect(ext.startsWith(".")).toBe(false);
    }
  });

  // Extensão repetida dentro do MESMO app entraria duas vezes na lista da rota
  // e o app apareceria duplicado no seletor de associações.
  it("nenhum app lista a mesma extensão duas vezes", () => {
    for (const app of CATALOG) {
      expect(new Set(app.extensions).size).toBe(app.extensions.length);
    }
  });
});
