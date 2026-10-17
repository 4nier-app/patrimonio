//! Perfis: cada pessoa da família tem o próprio cofre, com a própria senha.
//! O perfil principal usa a pasta de dados de sempre (quem já usava o app não
//! precisa mudar nada); os outros ficam em `perfis/<id>`. A lista com os
//! nomes fica em `perfis.json`, fora dos cofres, para a tela de entrada
//! mostrar antes da senha (só os nomes, nenhum dado da carteira).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MAIN: &str = "principal";
const FILE: &str = "perfis.json";
const DIR: &str = "perfis";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub profiles: Vec<Profile>,
    /// Último perfil aberto: a tela de entrada começa nele.
    pub last: Option<String>,
}

impl Registry {
    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }
}

/// Lê a lista; sem arquivo (ou com ele estragado), só o perfil principal.
pub fn load(base: &Path) -> Registry {
    let mut r: Registry = fs::read(base.join(FILE))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if r.get(MAIN).is_none() {
        r.profiles.insert(
            0,
            Profile {
                id: MAIN.into(),
                name: "Perfil principal".into(),
            },
        );
    }
    r
}

fn save(base: &Path, r: &Registry) -> Result<(), String> {
    fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let json = serde_json::to_vec_pretty(r).map_err(|e| e.to_string())?;
    fs::write(base.join(FILE), json).map_err(|e| e.to_string())
}

/// Pasta do cofre do perfil.
pub fn dir_of(base: &Path, id: &str) -> PathBuf {
    if id == MAIN {
        base.to_path_buf()
    } else {
        base.join(DIR).join(id)
    }
}

fn clean_name(r: &Registry, id: Option<&str>, name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("informe o nome do perfil".into());
    }
    if name.chars().count() > 40 {
        return Err("use um nome com até 40 letras".into());
    }
    let taken = r
        .profiles
        .iter()
        .any(|p| Some(p.id.as_str()) != id && p.name.to_lowercase() == name.to_lowercase());
    if taken {
        return Err(format!("já existe um perfil chamado {name}"));
    }
    Ok(name.to_string())
}

pub fn create(base: &Path, name: &str) -> Result<Profile, String> {
    let mut r = load(base);
    let name = clean_name(&r, None, name)?;
    // Id só com letras e números, para servir de nome de pasta.
    let mut n = chrono::Utc::now().timestamp_millis();
    while r.get(&format!("p{n}")).is_some() {
        n += 1;
    }
    let p = Profile {
        id: format!("p{n}"),
        name,
    };
    r.profiles.push(p.clone());
    r.last = Some(p.id.clone());
    save(base, &r)?;
    Ok(p)
}

pub fn rename(base: &Path, id: &str, name: &str) -> Result<(), String> {
    let mut r = load(base);
    let name = clean_name(&r, Some(id), name)?;
    let p = r
        .profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or("perfil não encontrado")?;
    p.name = name;
    save(base, &r)
}

/// Remove da lista um perfil que ainda não tem cofre (criado por engano).
/// Perfil com cofre não se remove por aqui: os dados ficariam perdidos.
pub fn remove_empty(base: &Path, id: &str, has_vault: bool) -> Result<(), String> {
    if id == MAIN {
        return Err("o perfil principal não pode ser removido".into());
    }
    if has_vault {
        return Err("este perfil já tem dados; ele não pode ser removido".into());
    }
    let mut r = load(base);
    r.profiles.retain(|p| p.id != id);
    if r.last.as_deref() == Some(id) {
        r.last = Some(MAIN.into());
    }
    let _ = fs::remove_dir(dir_of(base, id));
    save(base, &r)
}

pub fn set_last(base: &Path, id: &str) -> Result<(), String> {
    let mut r = load(base);
    if r.get(id).is_none() {
        return Err("perfil não encontrado".into());
    }
    r.last = Some(id.into());
    save(base, &r)
}

/// Perfil em que o app abre: o último usado, ou o principal.
pub fn initial(base: &Path) -> String {
    let r = load(base);
    r.last
        .clone()
        .filter(|id| r.get(id).is_some())
        .unwrap_or_else(|| MAIN.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfis_com_pastas_separadas() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        assert_eq!(initial(base), MAIN);
        assert_eq!(dir_of(base, MAIN), base);
        let p = create(base, " Esposa ").unwrap();
        assert_eq!(p.name, "Esposa");
        assert_eq!(dir_of(base, &p.id), base.join("perfis").join(&p.id));
        assert_eq!(initial(base), p.id);
        assert!(create(base, "esposa").is_err());
        assert!(create(base, "  ").is_err());
        rename(base, MAIN, "Marcelo").unwrap();
        assert_eq!(load(base).get(MAIN).unwrap().name, "Marcelo");
        assert!(remove_empty(base, MAIN, false).is_err());
        assert!(remove_empty(base, &p.id, true).is_err());
        remove_empty(base, &p.id, false).unwrap();
        assert_eq!(load(base).profiles.len(), 1);
        assert_eq!(initial(base), MAIN);
    }
}
