use std::process::Command;

/// Découpe une ligne de commande en tokens, en gérant les guillemets
/// doubles pour les chemins contenant des espaces (ex: `"C:\Program
/// Files\App\app.exe" --flag`). Tokenizer volontairement simple --
/// suffisant pour le cas d'usage (un chemin + quelques arguments), ce
/// n'est pas un shell complet (pas d'échappement, pas de guillemets
/// imbriqués, etc.).
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in input.trim().chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Lance `command_line` comme un nouveau processus DÉTACHÉ (spawn, pas
/// output()/status()) : on ne veut ni attendre sa fin, ni capturer sa
/// sortie -- exactement le même effet qu'un double-clic sur un
/// exécutable dans l'explorateur de fichiers.
///
/// IMPORTANT -- pas de CREATE_NO_WINDOW ici, volontairement, contrairement
/// à services.rs : là-bas on masque la fenêtre parce que c'est un appel
/// interne (sc/systemctl) dont on capture juste la sortie. Ici,
/// l'utilisateur lance explicitement un programme de son choix -- s'il
/// s'agit d'une appli console, il s'attend probablement à voir sa
/// fenêtre, comme s'il l'avait lancée lui-même depuis l'explorateur.
pub fn run_task(command_line: &str) -> Result<(), String> {
    let tokens = tokenize(command_line);
    let Some((program, args)) = tokens.split_first() else {
        return Err("Aucune commande saisie.".to_string());
    };

    Command::new(program)
        .args(args)
        .spawn()
        .map(|_child| ())
        .map_err(|e| format!("Impossible de lancer \"{program}\" : {e}"))
}
