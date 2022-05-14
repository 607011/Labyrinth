const CMDNAMES = {
    RIDDLE: 'riddle',
    NORTH: 'north',
    EAST: 'east',
    SOUTH: 'south',
    WEST: 'west',
    WHOAMI: 'whoami',
    WHEREAMI: 'whereami',
    CLEAR: 'clear',
    PING: 'ping',
    CHEAT: 'cheat',
    HELP: 'help',
    HISTORY: 'history',
    ENABLE: 'enable',
    LOGIN: 'login',
    LOGOUT: 'logout',
    ACTIVATE: 'activate',
    ABOUT: 'about',
    TOS: 'tos',
    HIGHSCORE: 'highscore',
    REGISTER: 'register',
    PASSWD: 'passwd',
    PROMOTE: 'promote',
};
const COMMANDS = [
    {
        name: CMDNAMES.PROMOTE,
        roles: [ROLE.ADMIN],
        args: [
            {
                name: 'username',
                type: 'string',
            },
            {
                name: 'role',
                type: 'string',
            },
        ],
        description: tr('Einen User hochstufen, z.B. zum `Designer` oder `Admin`'),
        fn: async function(params) {
            let [username, role] = params;
            while (typeof username === 'undefined') {
                username = await this.getInput(tr('Benutzername: '), { match: RE.USERNAME });
            }
            while (typeof role === 'undefined') {
                role = await this.getInput(tr('Rolle: '), { match: RE.ROLE });
            }
            let reply = await authenticatedRequest(constructURL(Game.URL.ADMIN.PROMOTE, {username, role}))
                .then(result => result.json());
            if (reply.ok) {
                this.print(tr(`User ${reply.username} wurde zum ${reply.role} ernannt.`));
                this.print(tr(`Der User muss sich neu anmelden, damit die Änderung wirksam wird.`));
            }
            else {
                this.print(tr(`Beförderung fehlgeschlagen: ${reply.message}.`));
            }
        },
    },
    {
        name: CMDNAMES.RIDDLE,
        roles: [ROLE.ADMIN, ROLE.DESIGNER],
        args: [
            {
                name: 'level',
                type: 'integer',
            }
        ],
        description: tr('Ein Rätsel über seine Level-Nummer abrufen'),
        fn: async function(params) {
            let [level] = params;
            if (typeof level === 'undefined') {
                return Promise.reject();
            }
            level = level | 0;
            try {
                let riddle = this.riddleCache[level];
                if (!riddle) {
                    riddle = await Riddle.loadByLevel(level);
                    this.riddleCache[riddle.level] = riddle;
                }
                if (riddle.task) {
                    this.print(parseMarkdown(riddle.task));
                    this.term.container.appendChild(makeDownloadLinkMime(riddle.task, 'TASK.md', 'text/markdown'));
                }
                if (riddle.files) {
                    for (const f of riddle.files) {
                        switch (f.mimeType) {
                            case 'image/png':
                                // fall-through
                            case 'image/jpeg':
                                // fall-through
                            case 'image/jpg':
                                // fall-through
                            case 'image/gif':
                                // fall-through
                            case 'image/webp':
                                const img = document.createElement('img');
                                img.src = `${UPLOAD_FOLDER}/${f.uploadedName}`;
                                if (f.variants) {
                                    img.srcset = f.variants.map(v => `${UPLOAD_FOLDER}/${v.uploadedName} ${v.scale}x`).join(' ');
                                }
                                img.style = `max-width: ${f.width}px;`;
                                this.term.container.appendChild(img);
                                this.term.container.appendChild(makeDownloadLink(f));
                                break;
                            case 'image/svg':
                            case 'image/svg+xml':
                                let svgimg = document.createElement('img');
                                svgimg.src = `${UPLOAD_FOLDER}/${f.uploadedName}`;
                                this.term.container.appendChild(svgimg);
                                this.term.container.appendChild(makeDownloadLink(f));
                                break;
                            case 'audio/mp3':
                                // fall-through
                            case 'audio/flac':
                                const dataUrl = `${UPLOAD_FOLDER}/${f.uploadedName}`;
                                this.print(`<audio controls><source src="${dataUrl}" type="${f.mimeType}"></audio>`);
                                break;
                            case 'text/markdown':
                                this.print(parseMarkdown(atob(f.data)));
                                this.term.container.appendChild(makeDownloadLink(f));
                                break;
                            case 'text/plain':
                                // fall-through
                            case 'text/yaml':
                                this.term.container.appendChild(makeDownloadLink(f));
                                break;
                            case 'text/html':
                                const embed = document.createElement('embed');
                                embed.src = `${UPLOAD_FOLDER}/${f.uploadedName}`;
                                embed.type = f.mimeType;
                                this.term.container.appendChild(embed);
                                this.term.container.appendChild(makeDownloadLink(f));
                                break;
                            case 'application/octet-stream':
                                // fall-through
                            case 'application/json':
                                // fall-through
                            case 'application/zip':
                                this.term.container.appendChild(makeDownloadLink(f));
                                break;
                            case 'application/pdf':
                                const pdf = document.createElement('embed');
                                pdf.src = `${UPLOAD_FOLDER}/${f.uploadedName}`;
                                pdf.type = f.mimeType;
                                pdf.style = 'width: 100%; background-color: white';
                                this.term.container.appendChild(pdf);
                                this.term.container.appendChild(makeDownloadLink(f));
                                break;
                            case 'video/webm':
                            case 'video/mp4':
                                    // fall-through
                            default:
                                console.error(tr(`Unknown file type: ${f.mimeType}`));
                        }
                    }
                }
                if (riddle.credits) {
                    this.print(tr(`<br/><b>Credits</b>: ${riddle.credits.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')}`));
                }
            }
            catch (e) {
                console.error(e);
                this.print(tr('Der Server antwortet nicht. Bitte versuch es später noch einmal ...'));
                return Promise.reject()
            }
            return Promise.resolve();
        }
    },
    {
        name: CMDNAMES.NORTH,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('nach Norden gehen'),
        fn: async function() {
            const success = await this.stepThroughDoorway('n');
            return success ? Promise.resolve() : Promise.reject();
        }
    },
    {
        name: CMDNAMES.EAST,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('nach Osten gehen'),
        fn: async function() {
            const success = await this.stepThroughDoorway('e');
            return success ? Promise.resolve() : Promise.reject();
        }
    },
    {
        name: CMDNAMES.SOUTH,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('nach Süden gehen'),
        fn: async function() {
            const success = await this.stepThroughDoorway('s');
            return success ? Promise.resolve() : Promise.reject();
        }
    },
    {
        name: CMDNAMES.WEST,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('nach Westen gehen'),
        fn: async function() {
            const success = await this.stepThroughDoorway('w');
            return success ? Promise.resolve() : Promise.reject();
        }
    },
    {
        name: CMDNAMES.WHOAMI,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('Infos über den angemeldeten Benutzer anzeigen'),
        fn: async function() {
            if (this.user !== null) {
                const user = await this.whoami();
                this.print(tr(`Das System weiß Folgendes über dich:`));
                this.print(tr(`<b>Benutzername</b>                  ${user.username}`));
                this.print(tr(`<b>Level</b>                         ${user.level}`));
                this.print(tr(`<b>Punkte</b>                        ${user.score | 0} von <span class="max_score">???</span>`));
                this.print(tr(`<b>gelöste Rätsel</b>                ${user.solved.length} von <span class="num_riddles">???</span>`));
                this.print(tr(`<b>betretene Räume</b>               ${user.rooms_entered ? user.rooms_entered.length : '???'} von <span class="num_rooms">???</span>`));
                this.print(tr(`<b>Standort</b>                      ${user.in_room.coords} #${user.in_room.number} (ID: ${user.in_room.id.$oid})`));
                this.print(tr(`<b>Mailadresse</b>                   ${user.email}`));
                this.print(tr(`<b>Rolle</b>                         ${user.role}`));
                const options = { year: 'numeric', month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', timeZoneName: 'short' };
                this.print(tr(`<b>Account erstellt</b>              ${user.created ? new Date(1000*user.created).toLocaleString(this.locale, options) : ''}`));
                this.print(tr(`<b>Registrierung abgeschlossen</b>   ${user.registered ? new Date(1000*user.registered).toLocaleString(this.locale, options): '&lt;unbekannt&gt;'}`));
                this.print(tr(`<b>letzte Anmeldung</b>              ${user.last_login ? new Date(1000*user.last_login).toLocaleString(this.locale, options): '&lt;unbekannt&gt;'}`));
                this.print(tr(`<b>zweite Faktoren</b>               ${user.configured_2fa instanceof Array && user.configured_2fa.length > 0 ? user.configured_2fa.join(', ') : 'keine'}`));
                authenticatedRequest(constructURL(Game.URL.GAME.STATS, {gameid: this.user.in_room.game_id.$oid}))
                .then(result => result.json())
                .then(data => {
                    if (data.ok) {
                        if (data.num_rooms > 0) {
                            for (const node of this.term.shadowRoot.querySelectorAll('.num_rooms')) {
                                node.textContent = data.num_rooms;
                            }
                        }
                        if (data.num_riddles > 0) {
                            for (const node of this.term.shadowRoot.querySelectorAll('.num_riddles')) {
                                node.textContent = data.num_riddles;
                            }
                        }
                        if (data.max_score > 0) {
                            for (const node of this.term.shadowRoot.querySelectorAll('.max_score')) {
                                node.textContent = data.max_score;
                            }
                        }
                    } 
                });
            }
            else {
                this.print(tr(`Du musst angemeldet sein, um Infos über dich anzeigen lassen zu können. Bitte \`${CMDNAMES.LOGIN}\`, or \`${CMDNAMES.REGISTER}\`! Tippe \`${CMDNAMES.HELP} ${CMDNAMES.LOGIN}\` oder \`${CMDNAMES.HELP} ${CMDNAMES.REGISTER}\`, um mehr zu erfahren.`));
                return Promise.reject();
            }
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.WHEREAMI,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('Infos über den Raum anzeigen, in dem du dich befindest'),
        fn: async function() {
            if (this.user !== null) {
                this.print(tr(`Du bist in Raum ${this.user.in_room.number}.`));
                if (this.user.in_room.entry) {
                    this.print(tr(`Dies ist der Eingang zum Labyrinth.`));
                }
                const directions = (this.user.in_room.neighbors.length > 1)
                ? `${this.user.in_room.neighbors.slice(0, -1).map(neighbor => DIRECTIONS[neighbor.direction]).join(', ')} und ${DIRECTIONS[this.user.in_room.neighbors[this.user.in_room.neighbors.length-1].direction]}`
                : DIRECTIONS[this.user.in_room.neighbors[0].direction];
                const rand = Math.random();
                const msg = rand > 0.7
                ? tr(`Du siehst ${this.user.in_room.neighbors.length === 1 ? 'einen Durchgang' : 'Durchgänge'} nach ${directions}.`)
                : rand > 0.5 
                    ? tr(`Du kannst von hier aus nach ${directions} gehen.`)
                    : rand > 0.3
                        ? tr(`Es gibt ${this.user.in_room.neighbors.length === 1 ? 'einen Weg' : 'Wege'} nach ${directions}.`)
                        : tr(`In Richtung ${directions} kannst du weitergehen.`);
                this.print(msg);
                for (const neighbor of this.user.in_room.neighbors) {
                    const solved = this.user.solved.find(rid => rid.riddle_id.$oid === neighbor.riddle_id.$oid);
                    const dirEl = document.createElement('span');
                    dirEl.className = 'clickable-direction';
                    dirEl.textContent = DIRECTIONS[neighbor.direction];
                    const boundClickHandler = function() {
                        const DIR_CMD_MAP = {
                            n: CMDNAMES.NORTH,
                            e: CMDNAMES.EAST,
                            s: CMDNAMES.SOUTH,
                            w: CMDNAMES.WEST,
                        };
                        const cmd = DIR_CMD_MAP[neighbor.direction];
                        this.term.enter(cmd);
                        dirEl.removeEventListener('click', boundClickHandler);
                    }.bind(this);
                    dirEl.addEventListener('click', boundClickHandler);
                    const doorMsgEl = document.createElement('div');
                    doorMsgEl.appendChild(document.createTextNode(tr('Die Tür nach ')));
                    doorMsgEl.appendChild(dirEl);
                    doorMsgEl.appendChild(document.createTextNode(tr(` ist ${solved ? 'offen' : 'verschlossen'}.`)));
                    this.term.appendNode(doorMsgEl);
                }
                return Promise.resolve();
            }
            else {
                this.print(tr(`Du musst angemeldet sein, um Infos über deine Umgebung anzeigen zu können. Benutze dazu \`${CMDNAMES.LOGIN}\` – oder \`${CMDNAMES.REGISTER}\`, um einen neuen Account zu erstellen. Tippe \`${CMDNAMES.HELP} ${CMDNAMES.LOGIN}\` or \`${CMDNAMES.HELP} ${CMDNAMES.REGISTER}\`, um mehr zu erfahren.`));
                return Promise.reject();
            }
        }
    },
    {
        name: CMDNAMES.CLEAR,
        roles: [ROLE.ANON, ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('Konsole löschen'),
        fn: function() {
            this.term.clear();
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.PING,
        roles: [ROLE.ANON, ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('prüfen, ob der Server antwortet'),
        fn: async function() {
            try {
                const response = await fetch(Game.URL.PING, {
                    method: 'GET',
                    cache: 'no-cache',
                    mode: 'cors',
                });
                if (response.status === 200) {
                    this.print(tr('Juchhuhh! Der Server lebt :-)'));
                }
                else {
                    this.print(tr('Der Server hat nicht geantwortet. Bitte versuche es später wieder ...'));
                }
            }
            catch(e) {
                this.print(tr(`Unerwarteter Fehler beim Ping: ${e}`));
                return Promise.reject();
            }
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.CHEAT,
        roles: [ROLE.ANON, ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('es mal mit Mogeln versuchen ;-)'),
        fn: async function() {
            try {
                const reply = await authenticatedRequest(Game.URL.CHEAT)
                .then(response => response.json());
                this.print(tr(`${reply.status}: ${reply.message} 😉`));
            }
            catch (e) {
                this.print(tr(`Der Server hat nicht geantwortet (${e}). Bitte versuche es später wieder ...`));
                return Promise.reject();
            }
            return Promise.resolve();
        }
    },
    {
        name: CMDNAMES.HELP,
        roles: [ROLE.ANON, ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        args: [
            {
                name: 'command',
                type: 'optional_string',
            }
        ],
        description: tr('diese Hilfe ausgeben'),
        fn: function(params) {
            const role = this.user && this.user.role ? this.user.role : null;
            if (params instanceof Array && params.length > 0) {
                for (const param of params) {
                    const c = Game.COMMANDS.find(cc => cc.name === param);
                    if (c) {
                        const params = c.args
                        ? c.args.map(a => {
                            if (a.type.startsWith('optional')) {
                                return `[${a.name}]`;
                            }
                            return a.name;
                        }).join(' ').toUpperCase()
                        : [];
                        this.print(`<span class="b600">${c.name} ${params}</span>\n  ${c.description}`);
                    }
                    else {
                        this.print(tr(`keine Hilfe für \`${param}\` verfügbar`));
                    }
                }
            }
            else {
                const output = Game.COMMANDS.filter(cmd => cmd.roles.includes(role)).map(cmd => `<strong>${cmd.name}</strong>:\n  ${cmd.description}`).join('\n');
                this.print(output);
            }
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.HISTORY,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('die jüngst eingegebenen Kommandos auflisten'),
        fn: function() {
            this.print(this.term.history.join('\n'));
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.ENABLE,
        args: [
            {
                name: 'command',
                type: 'string',
            },
            {
                name: 'subcommand',
                type: 'string',
            },
        ],
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: '<span class="b500">Kommando</span> `2fa`: Zwei-Faktor-Authentifizierung einschalten.\n    Die erlaubten <span class="b500">Subkommandos</span> <a href="https://de.wikipedia.org/wiki/Time-based_One-time_Password_Algorithmus" target="_blank">totp</a> und <a href="https://de.wikipedia.org/wiki/FIDO2" target="_blank">fido2</a> stehen für die Authentifizierungsverfahren.',
        fn: async function(params) {
            let [command, subcommand] = params;
            if (this.user === null) {
                this.print(tr('Du musst angemeldet sein, um einen zweiten Faktor zu registrieren. `login`, dann noch mal versuchen, bitte!'));
                return Promise.reject();
            }
            switch (command) {
                case '2fa':
                    if (typeof subcommand === 'undefined') {
                        this.print(tr(`Bitte wähle eine Methode:\n${FACTORS.map((key, value) => `${1+value}.) ${key.toUpperCase()}`).join('\n')}`));
                        subcommand = await this.chooseSecondFactor(FACTORS);
                    }
                    switch (subcommand.toLowerCase()) {
                        case 'totp':
                            const totp_enable_reply = await authenticatedRequest(Game.URL.USER.TOTP.ENABLE, 'POST')
                            .then(response => response.json());
                            if (totp_enable_reply.totp) {
                                this.print(tr('\nHier ist dein <a href="https://www.bsi.bund.de/DE/Themen/Verbraucherinnen-und-Verbraucher/Informationen-und-Empfehlungen/Cyber-Sicherheitsempfehlungen/Accountschutz/Zwei-Faktor-Authentisierung/zwei-faktor-authentisierung_node.html" target="_blank">zweiter Faktor</a> als QR-Code für <a href="https://de.wikipedia.org/wiki/Time-based_One-time_Password_Algorithmus" target="_blank">TOTP</a>-Generatoren wie <a href="https://authy.com/" targer="_blank">Authy</a> oder <a href="https://de.wikipedia.org/wiki/Google_Authenticator" target="_blank">Google Authenticator</a>:'));
                                this.print();
                                if (totp_enable_reply.totp.qrcode) {
                                    const img = document.createElement('img');
                                    img.src = `data:image/png;base64,${totp_enable_reply.totp.qrcode}`;
                                    img.style.width = '256px';
                                    img.style.height = '256px';
                                    img.style.display = 'block';
                                    this.term.container.appendChild(img);
                                }
                                this.print(tr('\nDu brauchst ihn für spätere Logins, falls du nicht noch einen anderen zweiten Faktor konfigurierst (siehe `help enable`).'));
                                this.print(tr(`Mit folgendem Aufruf kannst du dir das TOTP auf der Kommandozeile generieren lassen: \n\`<span class="b500"><a href="https://www.nongnu.org/oath-toolkit/oathtool.1.html" target="_blank">oathtool</a> --totp=${totp_enable_reply.totp.hash} -s ${totp_enable_reply.totp.interval}s --digits=${totp_enable_reply.totp.digits} --base32 ${totp_enable_reply.totp.secret}</span>\``));
                            }
                            break;
                        case 'fido2':
                            if (!window.PublicKeyCredential) {
                                this.print('Dein Browser kann nicht mit FIDO2-Keys umgehen. Bitte besorge dir einen <a href="https://caniuse.com/?search=webauthn" target="_blank">Browser, der es kann</a>.');
                                return Promise.reject();
                            }
                            this.print(tr('Registrierungsanfrage wird gestellt ...'));
                            const reply = await this.enableFIDO2(this.user.username);
                            console.debug(reply);
                            if (reply.ok) {
                                this.print(tr(`Der Key wurde erfolgreich registriert. Du kannst ihn fortan als zweiten Faktor beim Login benutzen.`));
                            }
                            else {
                                this.print(tr(`Die Registrierung des Keys ist fehlgeschlagen${reply.message ? ` (${reply.message})` : ''}. Bitte versuchs noch einmal.`));
                            }
                            break;
                        default:
                            this.print(tr(`unbekanntes Unterkommando: ${subcommand}`));
                            return Promise.reject();
                    }
                    break;
                default:
                    this.print(tr(`unbekanntes Kommando: ${command}`));
                    return Promise.reject();
                }
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.LOGIN,
        roles: [ROLE.ANON],
        args: [
            {
                name: 'username',
                type: 'optional_string',
            },
            {
                name: 'password',
                type: 'optional_password',
            },
            {
                name: 'totp',
                type: 'optional_string',
            },
        ],
        description: tr('als Benutzer `USERNAME` anmelden'),
        fn: async function(params) {
            let [username, password, totp] = params;
            while (typeof username === 'undefined') {
                username = await this.getInput(tr('Benutzername: '), { match: RE.USERNAME });
            }
            while (typeof password === 'undefined') {
                password = await this.getInput(tr('Passwort: '), { password: true, match: RE.PASSWORD });
            }
            this.showProgressbar();
            this.print(tr(`Zugangsdaten werden geprüft. Bitte warten ...`));
            try {
                const response = await fetch(Game.URL.USER.LOGIN, {
                    method: 'POST',
                    cache: 'no-cache',
                    mode: 'cors',
                    credentials: 'same-origin',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({
                        username: username,
                        password: password,
                        totp: totp,
                    }),
                });
                const reply = await response.json();
                console.debug(reply);
                switch (response.status) {
                    case 200:
                        if (reply.ok) {
                            this.user = new User(reply);
                        }
                        else {
                            if (reply.mfaMethods instanceof Array && reply.mfaMethods.length > 0) {
                                let chosenMFA = reply.mfaMethods[0];
                                if (reply.mfaMethods.length > 1) {
                                    this.print(tr(`Du hast mehrere zweite Faktoren zur Authentifizierung konfiguriert.`));
                                    this.print(tr('Welchen möchtest du verwenden?'));
                                    this.print(reply.mfaMethods.map((value, idx) => `${idx+1}. ${value}`).join('<br/>'));
                                    chosenMFA = await this.chooseSecondFactor(reply.mfaMethods);
                                }
                                switch (chosenMFA) {
                                    case SECOND_FACTOR.TOTP:
                                        this.print(tr(`Du hast TOTP als zweiten Faktor gewählt. Bitte gib die PIN an, die dir dein TOTP-Generator anzeigt, um den Anmeldevorgang abzuschließen.`));
                                        totp = await this.getInput('TOTP? ', { match: RE.PIN });
                                        await this.loginTOTP(username, totp);
                                        break;
                                    case SECOND_FACTOR.FIDO2:
                                        this.print(tr(`Webauthn-Prozess läuft ...`));
                                        await this.loginFIDO2(username);
                                        break;
                                    default:
                                        this.print(tr(`Unbekannter zweiter Faktor konfiguriert: ${reply.mfaMethods[0]}`));
                                        break;
                                }
                            }
                            else {
                                this.print(tr(`Die Anmeldung ist fehlgeschlagen. Meldung vom Server: "${reply.message}".`));
                                return Promise.reject();
                            }
                        }
                        break;
                    default:
                        this.print(tr(`Die Anmeldung ist fehlgeschlagen: ${reply.message}.`));
                        return Promise.reject();
                }
            }
            catch (e) {
                this.print(tr('Login abgebrochen.'));
                return Promise.reject();
            }
            this.print(tr(`Das hat geklappt! Du bist nun als <i>${this.user.username}</i> angemeldet.`));
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.LOGOUT,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('den angemeldeten Benutzer abmelden'),
        fn: async function() {
            if (this.user === null) {
                this.print(tr('Du bist nicht angemeldet, also kannst du dich auch nicht abmelden 😉'));
                return Promise.reject();
            }
            this.logout();
            this.print(tr('Abgemeldet.'));
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.ACTIVATE,
        roles: [ROLE.ANON],
        description: tr('den eben gerade registrierten Benutzer per PIN-Eingabe aktivieren'),
        args: [
            {
                name: 'username',
                type: 'optional_string',
            },
            {
                name: 'pin',
                type: 'optional_string',
            },
        ],
        fn: async function(params) {
            if (this.user !== null) {
                this.print(tr(`Bitte erst mit \`${CMDNAMES.LOGOUT}\` abmelden!'`));
                return Promise.reject();
            }
            let [username, pin] = params;
            if (typeof username === 'undefined') {
                username = await this.getInput(tr('Benutzername: '), { match: RE.USERNAME });
            }
            if (typeof pin === 'undefined') {
                pin = await this.getInput(tr('PIN: '), { match: RE.PIN });
            }
            const userData = await this.activate(username, pin);
            if (userData.ok === false) {
                this.print('Das war nix. Entweder stimmte der Benutzername nicht oder die PIN - oder der Benutzer wurde bereits aktiviert.');
                return Promise.reject();
            }
            this.proceedWith2FA(userData);
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.ABOUT,
        roles: [ROLE.ANON, ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('Infos über diese Software anzeigen'),
        fn: async function() {
            const about = await fetch(`${window.location.href}/data/about-${this.locale}.md`).then(response => response.text(), {cache: 'no-store'});
            this.print(parseMarkdown(about));
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.TOS,
        roles: [ROLE.ANON, ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('Nutzungsbedingungen anzeigen'),
        fn: async function() {
            const tos = await fetch(`${window.location.href}/data/tos-${this.locale}.md`).then(response => response.text(), {cache: 'no-store'});
            this.print(parseMarkdown(tos));
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.HIGHSCORE,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('Highscores anzeigen'),
        fn: async function() {
            this.print('NOCH NICHT IMPLEMENTIERT');
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.PASSWD,
        roles: [ROLE.USER, ROLE.ADMIN, ROLE.DESIGNER],
        description: tr('Passwort ändern'),
        args: [
            {
                name: 'password',
                type: 'optional_password',
            },
        ],
        fn: async function(params) {
            let [password] = params;
            while (typeof password === 'undefined') {
                password = await this.getPassword();
            }
            const reply = await authenticatedRequest(Game.URL.USER.PASSWD, 'POST', {password}).then(response => response.json());
            if (reply.ok) {
                this.print(tr('Passwort geändert.'));
            }
            else {
                this.print(tr(`Ändern des Passworts fehlgeschlagen: ${reply.message}.`));
                return Promise.reject();
            }
            return Promise.resolve();
        },
    },
    {
        name: CMDNAMES.REGISTER,
        roles: [ROLE.ANON],
        args: [
            {
                name: 'username',
                type: 'optional_string',
            },
            {
                name: 'email',
                type: 'optional_string',
            },
            {
                name: 'password',
                type: 'optional_password',
            },
        ],
        description: tr(`dich selbst als \`USERNAME\` mit dem Passwort \`PASSWORD\` und der Mailadresse \`EMAIL\` für das Spiel registrieren. Du kannst nach erfolgreicher Registrierung mit \`${CMDNAMES.ENABLE} 2fa\` einen zweiten Faktor aktivieren.`),
        fn: async function(params) {
            let [username, email, password] = params;
            if (localStorage.getItem('tosAccepted') !== 'true') {
                this.print(tr('Bitte lies die Nutzungsbedingungen, bevor du fortfährst.'));
                const tos = await fetch(`${window.location.href}/data/tos-${this.locale}.md`).then(response => response.text(), {cache: 'no-store'});
                if (tos) {
                    this.print(parseMarkdown(tos));
                    this.print(tr('Akzeptierst du die Bedingungen?'))
                    let answer = '';
                    while (answer !== tr('ja')) {
                        answer = await this.getInput(tr('Tippe `ja` zum Akzeptieren! '));
                        if (answer === tr('nein')) {
                            this.print(tr('Kein Problem. Später vielleicht ...'));
                            return Promise.reject();
                        }
                    }
                    localStorage.setItem('tosAccepted', true);
                }
                else {
                    this.print(tr('Ups! Die Nutzungsbedingungen konnten nicht vom Server geladen werden.'));
                    return Promise.reject();
                }
            }
            if (typeof username === 'undefined') {
                username = await this.getInput(tr('Benutzername: '), { match: RE.USERNAME });
            }
            if (typeof email === 'undefined') {
                email = await this.getInput(tr('Mailadresse: '), { match: RE.EMAIL });
            }
            while (typeof password === 'undefined') {
                password = Game.getPassword();
            }
            let data = {
                username: username,
                email: email,
                password: password,
                locale: this.locale,
                secondFactorMethod: null,
            };
            this.showProgressbar();
            try {
                const response = await fetch(Game.URL.USER.REGISTER, {
                    method: 'POST',
                    cache: 'no-cache',
                    mode: 'cors',
                    credentials: 'same-origin',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify(data)
                });
                const msg = await response.json();
                switch (response.status) {
                    case 201:
                        if (!msg.ok) {
                            this.print(tr(`Es ist ein Fehler aufgetreten: ${msg.message}`));
                            return Promise.reject();
                        }
                        break;
                    case 400:
                        console.error(msg.message);
                        this.print(tr(`Fehler: ${msg.message}`))
                        return Promise.reject();
                    case 409:
                        this.print(tr(`Fehler bei der Registrierung: ${msg.message}`));
                        return Promise.reject();
                    default:
                        console.error(response.statusText);
                        this.print(tr(`Ein Fehler ist aufgetreten: ${response.statusText}`));
                        return Promise.reject();
                }
            }
            catch (e) {
                console.error(e);
                this.print(tr('Der Registrierungs-Service steht derzeit nicht zur Verfügung. Versuchs bitte später noch einmal ...'));
                return Promise.reject();
            }
            navigator.credentials.store(new PasswordCredential({
                type: 'password',
                id: username,
                password: password,
            }));
            this.print(tr(`Wir haben eine E-Mail mit der Aktivierungs-PIN an ${email} gesendet.`))
            let pin = await this.getInput(tr('Bitte gib die PIN ein, um die Registrierung abzuschließen: '), { match: RE.PIN });
            this.print();
            const userData = await this.activate(username, pin);
            if (userData.ok === false) {
                console.error(userData.message);
                this.print(`Fehler: ${userData.message}`);
                this.print(`Du kannst später noch einmal versuchen, deinen Account per \`${CMDNAMES.ACTIVATE}\` zu aktivieren ...`);
                return Promise.reject();
            }
            if (!userData) {
                this.print(`Irgendwas ist schiefgegangen. Versuch bitte später noch einmal, deinen Account per \`${CMDNAMES.ACTIVATE}\` zu aktivieren ...`);
                return Promise.reject();
            }
            this.user = new User(userData);
            this.print(tr(`Die Aktivierung war erfolgreich. Dein Konto wurde unter dem Namen <i>${userData.username}</i> angelegt.`));
            if (userData.recovery_keys) {
                this.print(tr(`Hier sind deine ${userData.recovery_keys.length} Wiederherstellungsschlüssel für den Fall, dass du dein Passwort vergisst:`));
                this.print();
                this.print(userData.recovery_keys.map((key, idx) => `  ${(idx+1).toString().padStart(2, ' ')}.  ${key}`).join('<br>'));
                this.print(tr('\nVerwahre sie bitte sicher, inklusive ihrer Folgenummer! Sie werden dir hier das erste und letzte Mal angezeigt.'));
            }
            return Promise.resolve();
        },
    },
];
