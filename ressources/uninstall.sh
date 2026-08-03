#!/bin/bash
# uninstall.sh — Désinstallateur pour SystemMonitor
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

echo "=== Désinstallation de $APP_NAME ==="

# --- Suppression du dossier d'installation ---
if [[ -d "$INSTALL_DIR" ]]; then
    echo "Suppression du dossier $INSTALL_DIR..."
    rm -rf "$INSTALL_DIR"
fi

# --- Suppression du lien symbolique ---
if [[ -e "$BIN_DIR/$APP_NAME" || -L "$BIN_DIR/$APP_NAME" ]]; then
    echo "Suppression du lien $BIN_DIR/$APP_NAME..."
    rm -f "$BIN_DIR/$APP_NAME"
fi

# --- Suppression de l'icône ---
if [[ -f "$ICON_DIR/$APP_NAME.png" ]]; then
    echo "Suppression de l'icône..."
    rm -f "$ICON_DIR/$APP_NAME.png"
fi

# --- Suppression du .desktop ---
if [[ -f "$DESKTOP_FILE" ]]; then
    echo "Suppression du fichier .desktop..."
    rm -f "$DESKTOP_FILE"
fi

# --- Mise à jour caches (best-effort) ---
echo "Mise à jour des caches..."
update-desktop-database /usr/share/applications 2>/dev/null || true
gtk-update-icon-cache /usr/share/icons/hicolor 2>/dev/null || true

echo "=== Désinstallation terminée ==="
