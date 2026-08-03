#!/bin/bash
# install.sh — Installateur universel tar.gz pour SystemMonitor
# Usage : ./install.sh   (depuis le dossier extrait de l'archive)
#
# Architecture : un seul binaire (SystemMonitor). Le kill de process
# appartenant à d'autres utilisateurs est autorisé via une capability
# Linux (CAP_KILL) attachée directement au binaire avec `setcap` -- pas
# de démon, pas de service systemd, pas d'utilisateur/groupe dédiés.
# Voir la note "Kill privilégié" plus bas pour le détail et les limites.

set -euo pipefail

APP_NAME="SystemMonitor"
INSTALL_DIR="/opt/$APP_NAME"
BIN_DIR="/usr/local/bin"
ICON_DIR="/usr/share/icons/hicolor/256x256/apps"
DESKTOP_FILE="/usr/share/applications/$APP_NAME.desktop"

# --- Re-exécution en root (une seule demande de mot de passe) ---
if [[ $EUID -ne 0 ]]; then
    exec sudo bash "$0" "$@"
fi

# --- Résolution du dossier du script (marche même si appelé via un chemin relatif ou un symlink) ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
cd "$SCRIPT_DIR"

echo "=== Installation de $APP_NAME ==="
echo "Dossier source : $SCRIPT_DIR"

# --- Vérification des fichiers requis avant de toucher au système ---
for f in "$APP_NAME" "Icone.png"; do
    if [[ ! -f "$f" ]]; then
        echo "Erreur : fichier '$f' introuvable dans $SCRIPT_DIR" >&2
        exit 1
    fi
done

# --- Détection de la distribution ---
DISTRO=""
DISTRO_LIKE=""
if [[ -f /etc/os-release ]]; then
    # shellcheck source=/etc/os-release
    source /etc/os-release
    DISTRO="${ID:-}"
    DISTRO_LIKE="${ID_LIKE:-}"
elif command -v lsb_release >/dev/null 2>&1; then
    DISTRO=$(lsb_release -si | tr '[:upper:]' '[:lower:]')
else
    echo "Impossible de détecter la distribution" >&2
    exit 1
fi

# --- Normalisation (utilise ID puis retombe sur ID_LIKE) ---
case "$DISTRO" in
    ubuntu|debian) DISTRO="debian" ;;
    fedora|rhel|centos) DISTRO="fedora" ;;
    opensuse*|suse) DISTRO="opensuse" ;;
    arch|manjaro|endeavouros) DISTRO="arch" ;;
    *)
        case "$DISTRO_LIKE" in
            *ubuntu*|*debian*) DISTRO="debian" ;;
            *fedora*|*rhel*) DISTRO="fedora" ;;
            *suse*) DISTRO="opensuse" ;;
            *arch*) DISTRO="arch" ;;
        esac
        ;;
esac
echo "Distribution détectée : ${DISTRO:-inconnue}"

# --- Dépendances système ---
# Les noms de paquets libX11/pango/cairo diffèrent selon la distro
# (suffixes SONAME sur Debian, casse différente sur Fedora, etc.).
# libcap2-bin/libcap fournit la commande `setcap`, nécessaire plus bas
# pour le kill privilégié.
case "$DISTRO" in
    debian)
        DEPS="libx11-6 libxext6 libxfixes3 libxft2 libfontconfig1 libpango-1.0-0 libcairo2 libcap2-bin"
        apt update
        apt install -y $DEPS
        ;;
    fedora)
        DEPS="libX11 libXext libXfixes libXft fontconfig pango cairo libcap"
        dnf install -y $DEPS
        ;;
    arch)
        DEPS="libx11 libxext libxfixes libxft fontconfig pango cairo libcap"
        pacman -Sy --noconfirm --needed $DEPS
        ;;
    opensuse)
        DEPS="libX11-6 libXext6 libXfixes3 libXft2 fontconfig pango cairo libcap-progs"
        zypper --non-interactive install $DEPS
        ;;
    *)
        echo "Distribution non reconnue automatiquement : installation des dépendances ignorée."
        echo "Assure-toi d'avoir les libs X11/Xext/Xfixes/Xft, fontconfig, pango, cairo"
        echo "et l'outil 'setcap' (paquet libcap/libcap2-bin selon la distro) installés."
        ;;
esac

# --- Copie du binaire ---
echo "Copie des fichiers dans $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
install -m 755 "$APP_NAME" "$INSTALL_DIR/$APP_NAME"

# --- Lien symbolique ---
ln -sf "$INSTALL_DIR/$APP_NAME" "$BIN_DIR/$APP_NAME"

# --- Icône ---
mkdir -p "$ICON_DIR"
install -m 644 "Icone.png" "$ICON_DIR/$APP_NAME.png"

# --- Fichier .desktop ---
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Name=$APP_NAME
Exec=$APP_NAME
Icon=$APP_NAME
Type=Application
Categories=Utility;System;
StartupNotify=true
EOF
chmod 644 "$DESKTOP_FILE"

# --- Kill privilégié : capability CAP_KILL attachée au binaire ---
# Permet à SystemMonitor d'envoyer un signal (kill) à des process
# n'appartenant pas à l'utilisateur qui le lance, sans root, sans
# setuid, sans démon séparé. C'est le même mécanisme que celui utilisé
# par `ping` pour CAP_NET_RAW.
#
# ATTENTION - implication de sécurité à connaître : cette capability
# s'applique à TOUT utilisateur qui exécute ce binaire, pas seulement
# à toi. Sur une machine mono-utilisateur (cas courant pour un poste
# perso Arch Linux), c'est un non-problème. Sur une machine partagée
# entre plusieurs comptes non-fiables, ce mécanisme donnerait à
# n'importe quel utilisateur du système la capacité de tuer les
# process des autres -- à garder en tête si ce contexte change un jour.
#
# NOTE : setcap doit être ré-appliqué à chaque mise à jour du binaire
# (une copie via `cp`/`install` perd la capability), d'où sa présence
# ici et non ailleurs.
if command -v setcap >/dev/null 2>&1; then
    setcap 'cap_kill=+ep' "$INSTALL_DIR/$APP_NAME"
    echo "Capability CAP_KILL appliquée sur $INSTALL_DIR/$APP_NAME."
else
    echo "Attention : commande 'setcap' introuvable, le kill de process"
    echo "d'autres utilisateurs ne fonctionnera pas (seuls tes propres"
    echo "process pourront être tués depuis l'application)."
fi

# --- Mise à jour des caches (best-effort, ne doit pas faire échouer l'install) ---
update-desktop-database /usr/share/applications 2>/dev/null || true
gtk-update-icon-cache /usr/share/icons/hicolor 2>/dev/null || true

echo "=== Installation terminée avec succès ==="
echo "Lance l'application avec : $APP_NAME"
