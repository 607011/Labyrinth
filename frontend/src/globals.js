const HOST = (function(window) {
    switch (window.location.hostname) {
        case 'labyrinth.raetselonkel.de': return 'https://labyrinth.raetselonkel.de/v1/api';
        default: return 'http://127.0.0.1:18080';
    }
})(window);
const UPLOAD_FOLDER = (function(window) {
    switch (window.location.hostname) {
        case 'labyrinth.raetselonkel.de': return 'https://labyrinth.raetselonkel.de/external/upload';
        default: return 'http://127.0.0.1:8080/Labyrinth/frontend/dist/upload';
    }
})(window);
// TODO: localise strings
const tr = (text) => text;
const RE = {
    EMAIL: /(?:[a-z0-9!#$%&'*+\/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+\/=?^_`{|}~-]+)*|"(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21\x23-\x5b\x5d-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])*")@(?:(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?|\[(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?|[a-z0-9-]*[a-z0-9]:(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21-\x5a\x53-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])+)\])/gm,
    PIN: /\d{6}/,
    USERNAME: /\w+/,
    PASSWORD: /.{8,}/,
    COMMANDLINE: /[^']+'|"[^"]+"|[^\s"]+/g,
    ROLE: /User|Admin|Designer/i,
};
const SECOND_FACTOR = {
    TOTP: 'TOTP',
    FIDO2: 'FIDO2',
};
const ROLE = {
    ANON: null,
    ADMIN: 'Admin',
    USER: 'User',
    DESIGNER: 'Designer',
};
const DIRECTIONS = {
    n: tr('Norden'),
    e: tr('Osten'),
    s: tr('Süden'),
    w: tr('Westen'),
};
