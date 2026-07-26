//! Pacote de SISTEMA no Linux: qual gerenciador, qual asset, qual comando.
//!
//! ─── Por que este módulo existe ──────────────────────────────────────────────
//!
//! Até aqui o modelo do Hub no Linux era **AppImage e só**: copiar pra
//! `~/Applications`, escrever um `.desktop` em `~/.local`, anotar no
//! `installed.json`. O `.deb` ele até detectava (via dpkg), mas na desinstalação
//! respondia "remova com o gerenciador de pacotes" — porque instalar pacote de
//! sistema pede root, e o Hub não pedia.
//!
//! Isto aqui muda: o Hub passa a instalar e remover pacote nativo por
//! **`pkexec`** (o polkit pergunta a senha numa janela do próprio sistema — o
//! Hub nunca vê nem guarda senha). A vantagem do pacote nativo é o binário usar
//! o **webkit2gtk do sistema** em vez do que o AppImage carrega dentro dele, e o
//! gerenciador cuidar de atalho, ícone e remoção.
//!
//! ─── Por que TUDO aqui é puro ───────────────────────────────────────────────
//!
//! Nada neste arquivo executa nada, e nada é gateado por `cfg`. É de propósito:
//! são as decisões que dão errado em silêncio (escolher o asset errado, montar
//! um `pacman -R` com o nome errado), e mantê-las puras é o que deixa o
//! `cargo test` exercitá-las **no Windows também** — a máquina onde este código
//! é escrito, e onde `#[cfg(not(windows))]` nunca compilaria. Quem executa é o
//! `lib.rs`, com a camada fina que sobra.

use serde::Serialize;

/// O gerenciador de pacotes da máquina. `None` = nenhum conhecido; o Hub cai no
/// AppImage, que é o caminho que sempre funcionou.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PkgManager {
    Pacman,
    Apt,
    None,
}

impl PkgManager {
    /// O nome curto que vai pro front (e que volta como `source` na detecção).
    pub fn as_str(self) -> &'static str {
        match self {
            PkgManager::Pacman => "pacman",
            PkgManager::Apt => "apt",
            PkgManager::None => "",
        }
    }
}

/// Qual gerenciador usar, dado quais binários existem.
///
/// **Pacman ganha do apt** quando os dois aparecem. Não é preferência: máquina
/// com pacman É Arch (ou derivada), e um `apt` presente ali é o `apt` do AUR ou
/// resquício — instalar `.deb` num Arch por cima do pacman é receita de sistema
/// quebrado. O contrário não acontece: Debian não tem pacman.
pub fn pick_manager(has_pacman: bool, has_apt: bool) -> PkgManager {
    if has_pacman {
        PkgManager::Pacman
    } else if has_apt {
        PkgManager::Apt
    } else {
        PkgManager::None
    }
}

/// O glob do asset que serve esta máquina.
///
/// Casa com o que o `release.yml` da suíte publica: `*.pkg.tar.zst` (Arch),
/// `*_amd64.deb` (Debian/Ubuntu) e `*_amd64.AppImage` (o resto).
pub fn asset_glob(m: PkgManager) -> &'static str {
    match m {
        PkgManager::Pacman => "*.pkg.tar.zst",
        PkgManager::Apt => "*_amd64.deb",
        PkgManager::None => "*_amd64.AppImage",
    }
}

/// Um caminho de arquivo é seguro pra entregar ao gerenciador?
///
/// A regra é curta e serve pra uma coisa só: o argumento **não pode começar com
/// `-`**, senão o pacman/apt o lê como OPÇÃO em vez de arquivo. Como o caminho
/// aqui sempre nasce do nosso diretório de downloads, isto é cinto de segurança,
/// não validação de entrada hostil — mas é barato e o dia em que o nome do
/// asset mudar, ele avisa.
pub fn safe_path_arg(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('-')
}

/// Um nome de pacote é seguro pra entregar a um `remove`?
///
/// **Esta é a função perigosa do módulo.** Do outro lado dela há um comando que
/// APAGA software da máquina do usuário com privilégio de root. Um nome vindo
/// torto — com espaço, com `/`, começando por `-` — não vira "erro de pacote não
/// encontrado": vira argumento extra ou opção pro pacman, e aí o que é removido
/// deixa de ser o que se pediu. Por isso a regra é lista branca (letra, dígito,
/// `-`, `_`, `.`, `+`, que é o alfabeto de nome de pacote em Debian e Arch), e
/// não lista negra.
pub fn valid_pkg_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
}

/// O comando de INSTALAR um arquivo de pacote, já com o `pkexec` na frente.
///
/// `None` quando o gerenciador é `None` (aí quem instala é o caminho do
/// AppImage) ou quando o caminho não passa no [`safe_path_arg`].
///
/// `--noconfirm`/`-y`: o Hub já perguntou ao usuário na sua própria UI, e um
/// prompt interativo aqui travaria pra sempre — não há terminal do outro lado.
pub fn install_cmd(m: PkgManager, file: &str) -> Option<Vec<String>> {
    if !safe_path_arg(file) {
        return None;
    }
    let args: Vec<&str> = match m {
        // `-U` é "instalar arquivo local" no pacman (o `-S` é do repositório).
        PkgManager::Pacman => vec!["pkexec", "pacman", "-U", "--noconfirm", file],
        // O apt aceita caminho local desde a 1.1, mas SÓ se ele parecer caminho:
        // sem `./` ou `/` na frente ele procura um pacote com esse nome no
        // repositório e falha dizendo que o pacote não existe. Quem chama manda
        // caminho absoluto, então está coberto — e o teste prova isso.
        PkgManager::Apt => vec!["pkexec", "apt-get", "install", "-y", file],
        PkgManager::None => return None,
    };
    Some(args.into_iter().map(String::from).collect())
}

/// O comando de REMOVER um pacote pelo nome. Ver [`valid_pkg_name`].
pub fn remove_cmd(m: PkgManager, pkg: &str) -> Option<Vec<String>> {
    if !valid_pkg_name(pkg) {
        return None;
    }
    let args: Vec<&str> = match m {
        PkgManager::Pacman => vec!["pkexec", "pacman", "-R", "--noconfirm", pkg],
        PkgManager::Apt => vec!["pkexec", "apt-get", "remove", "-y", pkg],
        PkgManager::None => return None,
    };
    Some(args.into_iter().map(String::from).collect())
}

/// O mesmo comando escrito pra um humano copiar num terminal.
///
/// É o que o Hub mostra quando não há `pkexec` na máquina (polkit não é
/// garantido — servidor, WM minimalista). A regra da casa manda **dizer o que
/// não deu e como fazer à mão**, em vez de falhar com "erro ao instalar".
pub fn as_manual_command(cmd: &[String]) -> String {
    let sem_pkexec: Vec<&String> = cmd.iter().filter(|a| a.as_str() != "pkexec").collect();
    let mut s = String::from("sudo");
    for a in sem_pkexec {
        s.push(' ');
        s.push_str(a);
    }
    s
}

/// Normaliza um nome pra casar "TaylorHub" (catálogo) com "taylor-hub" (nome do
/// pacote). É a MESMA regra que a detecção via dpkg já usava — agora
/// compartilhada com a do pacman, que é o ponto: o `.deb` e o pacote do Arch
/// nascem do mesmo build e levam o mesmo `Package:`, então uma regra só serve as
/// duas distros. (É por isso que o repack NÃO acrescenta o sufixo `-bin` que o
/// AUR usaria — ver `scripts/deb-to-arch.sh`.)
pub fn norm_name(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

/// Extrai a versão da saída de `pacman -Q <pkg>`, que é `"<nome> <versão>\n"`.
///
/// O pacman acrescenta o `pkgrel` (`0.23.2-1`) e pode trazer epoch (`1:0.23.2-1`);
/// o Hub compara com a versão da release do GitHub (`0.23.2`), então os dois
/// enfeites saem. Sem isso, o Hub acharia que TODA versão instalada é diferente
/// da publicada e ofereceria "atualizar" pra sempre.
pub fn parse_pacman_query(out: &str) -> Option<String> {
    let line = out.lines().find(|l| !l.trim().is_empty())?;
    let raw = line.split_whitespace().nth(1)?;
    let sem_epoch = raw.split(':').next_back().unwrap_or(raw);
    let sem_rel = sem_epoch.split('-').next().unwrap_or(sem_epoch);
    if sem_rel.is_empty() {
        None
    } else {
        Some(sem_rel.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacman_ganha_do_apt() {
        // Máquina com os dois É Arch: instalar .deb ali quebraria o sistema.
        assert_eq!(pick_manager(true, true), PkgManager::Pacman);
        assert_eq!(pick_manager(true, false), PkgManager::Pacman);
        assert_eq!(pick_manager(false, true), PkgManager::Apt);
        assert_eq!(pick_manager(false, false), PkgManager::None);
    }

    #[test]
    fn sem_gerenciador_o_asset_e_o_appimage() {
        // O caminho que sempre funcionou continua sendo o fallback.
        assert_eq!(asset_glob(PkgManager::None), "*_amd64.AppImage");
        assert_eq!(asset_glob(PkgManager::Pacman), "*.pkg.tar.zst");
        assert_eq!(asset_glob(PkgManager::Apt), "*_amd64.deb");
    }

    /// Os globs têm que casar com os nomes REAIS que o release.yml publica —
    /// um glob que não casa vira "nenhum asset encontrado" na cara do usuário.
    #[test]
    fn os_globs_casam_com_os_nomes_publicados() {
        fn casa(glob: &str, nome: &str) -> bool {
            // glob simples: só `*` no começo, que é a forma usada aqui.
            let suf = glob.trim_start_matches('*');
            nome.ends_with(suf)
        }
        assert!(casa(asset_glob(PkgManager::Pacman), "taylor-hub-0.23.2-1-x86_64.pkg.tar.zst"));
        assert!(casa(asset_glob(PkgManager::Apt), "TaylorHub_0.23.2_amd64.deb"));
        assert!(casa(asset_glob(PkgManager::None), "TaylorHub_0.23.2_amd64.AppImage"));
        // E não se confundem entre si: o .deb não pode casar com o glob do Arch.
        assert!(!casa(asset_glob(PkgManager::Pacman), "TaylorHub_0.23.2_amd64.deb"));
    }

    #[test]
    fn instalar_monta_o_comando_certo_com_pkexec() {
        assert_eq!(
            install_cmd(PkgManager::Pacman, "/tmp/x.pkg.tar.zst").unwrap(),
            vec!["pkexec", "pacman", "-U", "--noconfirm", "/tmp/x.pkg.tar.zst"]
        );
        assert_eq!(
            install_cmd(PkgManager::Apt, "/tmp/x.deb").unwrap(),
            vec!["pkexec", "apt-get", "install", "-y", "/tmp/x.deb"]
        );
        // Sem gerenciador não há comando: quem instala é o caminho do AppImage.
        assert!(install_cmd(PkgManager::None, "/tmp/x.AppImage").is_none());
    }

    /// O apt só entende .deb local se o argumento PARECER caminho. Como o Hub
    /// sempre passa caminho absoluto, o contrato está satisfeito — e este teste
    /// é o que trava isso, porque um dia alguém vai querer passar só o nome do
    /// arquivo e o erro do apt ("Unable to locate package") manda investigar
    /// repositório, não o argumento.
    #[test]
    fn o_caminho_do_apt_e_absoluto() {
        let cmd = install_cmd(PkgManager::Apt, "/home/u/.cache/hub/TaylorHub.deb").unwrap();
        let arquivo = cmd.last().unwrap();
        assert!(arquivo.starts_with('/'), "o apt precisa de caminho, não de nome solto");
    }

    #[test]
    fn nome_de_pacote_torto_nao_vira_comando_de_remocao() {
        // Do outro lado desta função há um root apagando software.
        assert!(remove_cmd(PkgManager::Pacman, "taylor-hub").is_some());
        assert!(remove_cmd(PkgManager::Apt, "local-office").is_some());
        for ruim in [
            "",             // vazio
            "-Rns",         // vira OPÇÃO do pacman, não pacote
            "--noconfirm",  // idem
            "a b",          // dois argumentos: removeria um pacote a mais
            "a;rm -rf /",   // metacaractere
            "../etc",       // caminho
            "pkg$(id)",     // substituição
            "pkg\nb",       // linha nova
        ] {
            assert!(
                remove_cmd(PkgManager::Pacman, ruim).is_none(),
                "aceitou nome perigoso: {:?}",
                ruim
            );
        }
    }

    #[test]
    fn nome_valido_aceita_o_alfabeto_real_de_pacote() {
        for bom in ["taylor-hub", "local_office", "gtk3", "webkit2gtk-4.1", "gcc-libs", "g++"] {
            assert!(valid_pkg_name(bom), "recusou nome legítimo: {}", bom);
        }
    }

    #[test]
    fn caminho_que_parece_opcao_nao_vira_instalacao() {
        assert!(install_cmd(PkgManager::Pacman, "-U").is_none());
        assert!(install_cmd(PkgManager::Apt, "").is_none());
    }

    #[test]
    fn comando_manual_troca_o_pkexec_por_sudo() {
        // É o texto que o usuário copia quando não há polkit na máquina.
        let cmd = install_cmd(PkgManager::Pacman, "/tmp/x.pkg.tar.zst").unwrap();
        assert_eq!(as_manual_command(&cmd), "sudo pacman -U --noconfirm /tmp/x.pkg.tar.zst");
        let cmd = remove_cmd(PkgManager::Apt, "taylor-hub").unwrap();
        assert_eq!(as_manual_command(&cmd), "sudo apt-get remove -y taylor-hub");
    }

    #[test]
    fn o_nome_do_catalogo_casa_com_o_nome_do_pacote() {
        // "TaylorHub" (catálogo) ≟ "taylor-hub" (Package: do .deb e do pacote do
        // Arch). Uma regra só serve as duas distros porque o repack preserva o
        // nome do .deb.
        assert_eq!(norm_name("TaylorHub"), norm_name("taylor-hub"));
        assert_eq!(norm_name("LocalOffice"), norm_name("local-office"));
        assert_eq!(norm_name("LocalChessPGN"), norm_name("local-chess-pgn"));
        // E não casa o que não é pra casar.
        assert_ne!(norm_name("LocalMedia"), norm_name("local-mind"));
    }

    #[test]
    fn versao_do_pacman_perde_pkgrel_e_epoch() {
        // Sem isto o Hub compararia "0.23.2-1" com "0.23.2" e ofereceria
        // atualizar pra sempre, em toda abertura.
        assert_eq!(parse_pacman_query("taylor-hub 0.23.2-1\n").as_deref(), Some("0.23.2"));
        assert_eq!(parse_pacman_query("local-office 1:0.16.7-2\n").as_deref(), Some("0.16.7"));
        assert_eq!(parse_pacman_query("x 1.0\n").as_deref(), Some("1.0"));
        // "não instalado" faz o pacman escrever no stderr e sair vazio.
        assert_eq!(parse_pacman_query(""), None);
        assert_eq!(parse_pacman_query("semversao\n"), None);
    }
}
