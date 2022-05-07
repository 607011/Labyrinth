class Game extends HTMLElement {
    static URL = {
        USER: {
            REGISTER: `${HOST}/user/register`,
            ACTIVATE: `${HOST}/user/activate`,
            LOGIN: `${HOST}/user/login`,
            LOGOUT: `${HOST}/user/logout`,
            AUTH: `${HOST}/user/auth`,
            WHOAMI: `${HOST}/user/whoami`,
            TOTP: {
                LOGIN: `${HOST}/user/totp/login`,
                ENABLE: `${HOST}/user/totp/enable`,
            },
            WEBAUTHN: {
                REGISTER: {
                    START: `${HOST}/user/webauthn/register/start`,
                    FINISH: `${HOST}/user/webauthn/register/finish`,
                },
                LOGIN: {
                    START: `${HOST}/user/webauthn/login/start/:username`,
                    FINISH: `${HOST}/user/webauthn/login/finish/:username`,
                },
            },
        },
        GO: `${HOST}/go/:direction`,
        GAME: {
            STATS: `${HOST}/game/stats/:gameid`,
        },
        PING: `${HOST}/ping`,
        CHEAT: `${HOST}/cheat`,
    };
    static COMMANDS = COMMANDS;
    /**
     * @constructor
     */
    constructor() {
        super();
        this._user = null;
        this.online = false;
        const shadowRoot = this.attachShadow({ mode: 'open' });
        this.header = document.createElement('header');
        this.header.innerHTML = `<span class="b500" id="server-state">-</span> | <span class="b500" id="login-state">nicht angemeldet</span>`;
        let style = document.createElement('style');
        style.textContent = 
`header {
    color: #090b13;
    background-color: #24cdd3;
    padding: 4px;
    box-shadow: 0 0 10px #2fa8b86e;
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 20px;
    z-index: 100;
}
terminal-div {
    position: absolute;
    top: 24px;
    width: calc(100% - 8px);
    // height: calc(100% - 40px);
    padding: 4px;
}
`;
        this.term = document.createElement('terminal-div');
        this.progressbar = document.createElement('progress-bar');
        this.progressbar.setAttribute('disabled', true);
        let container = document.createElement('div');
        this.header.appendChild(this.progressbar);
        container.appendChild(this.header);
        container.appendChild(this.term);
        shadowRoot.appendChild(style);
        shadowRoot.appendChild(container);
        this.server_state_el = shadowRoot.querySelector('#server-state');
        this.login_state_el = shadowRoot.querySelector('#login-state');
        this.riddleCache = {};
        // this.locale = navigator.language || 'de-DE';
        this.locale = 'de-DE';
        window.addEventListener('score', e => { 
            shadowRoot.querySelector('#score').textContent = e.detail.score;
         });
        window.addEventListener('level', e => { 
            shadowRoot.querySelector('#level').textContent = e.detail.level;
        });
    }
    static get observedAttributes() { return []; }
    connectedCallback() {
    }
    disconnectedCallback() {
    }
    attributeChangedCallback(attrName, oldVal, newVal) {
    }
    showProgressbar(param) {
        if (param && param.maximized) {
            this.progressbar.update({ min: 0, max: 1, value: 1 })
        }
        this.progressbar.setAttribute('disabled', false)
    }
    hideProgressbar() {
        this.progressbar.setAttribute('disabled', true)
    }
    /**
     * Prompt the user for input, then receive input.
     * @param {String} prompt 
     * @param {object} params 
     * @returns {String}
     */
    async getInput(prompt, params) {
        let input;
        this.hideProgressbar();
        while (typeof input === 'undefined') {
            this.term.write(prompt);
            input = await this.term.waitForInput(params);
            if (input === '') {
                break;
            }
            else if (params && params.match) {
                if (params.match instanceof RegExp && !input.match(params.match)) {
                    break;
                }
                else if (params.match instanceof Array && params.match.includes(input)) {
                    break;
                }
                else if (typeof params.match === 'string' && params.match === input) {
                    break;
                }
            }
            this.print();
        }
        return input;
    }
    async ping() {
        this.server_state_el.textContent = 'OFFLINE';
        this.online = false;
        try {
            let response = await fetch(Game.URL.PING, {
                method: 'GET',
                cache: 'no-cache',
                mode: 'cors',
            });
            if (response.status === 200) {
                this.online = true;
                const reply = await response.json();
                this.server_state_el.textContent = `ONLINE (v${reply.version})`;
            }
        }
        catch (e) {
            console.error(e, this.server_state_el);
        }
    }
    /**
     * main()
     */
    async play() {
        this.print('<b>*****************************</b>');
        this.print('<b>****      LABYRINTH      ****</b>');
        this.print('<b>*****************************</b>');
        this.print(tr(`Initialisierung benötigte ${(performance.now()-t0).toFixed(1)} Millisekunden.`));
        this.term.write(tr(`Prüfen, ob Server online ist ... `));
        this.showProgressbar();
        await this.ping();
        this.print(this.online ? tr('ja.') : tr('nein.'));
        if (this.loginDataPresent()) {
            const jwt = localStorage.getItem('jwt');
            const parsedJWT = User.parsedJWT(jwt);
            const username = parsedJWT.sub;
            const expires = parsedJWT.exp | 0;
            const expires_in_secs = Math.floor(expires - currentUnixTimestamp());
            const expires_in_days = Math.floor(expires_in_secs / 60 / 60 / 24);
            const expires_in_hours = Math.floor(expires_in_secs / 60 / 60);
            const expires_in_mins = Math.floor(expires_in_secs / 60);
            const validity = (expires_in_days === 0)
            ? (expires_in_hours === 0)
                ? (expires_in_mins === 0)
                    ? tr(`${expires_in_secs} Sekunde${expires_in_secs === 1 ? '' : 'n'}`) : tr(`${expires_in_mins} Minute${expires_in_mins === 1 ? '' : 'n'}`)
                : tr(`${expires_in_hours} Stunde${expires_in_hours === 1 ? '' : 'n'}`)
            : tr(`${expires_in_days} Tag${expires_in_days === 1 ? '' : 'en'}`);
            this.print(tr(`Authentifizierungs-Token für User <i>${username}</i> gefunden, das in in ${validity} (${new Date(expires*1000).toLocaleString(this.locale)}) abläuft.`));
            this.term.write(tr(`Prüfen des Tokens ... `));
            try {
                const response = await authenticatedRequest(Game.URL.USER.AUTH);
                if (response.status === 200) {
                    const user = await this.whoami();
                    if (user.username) {
                        this.user = user;
                        this.role = parsedJWT.role;
                        this.print(tr(`gültig. Du bist nun eingeloggt.`));
                    }
                    else {
                        this.print(tr(`User nicht gefunden.`));
                        this.user = null;
                        this.role = null;
                    }
                }
                else {
                    this.login_state_el.innerText = tr(`nicht angemeldet`);
                    this.user = null;
                    this.role = null;
                }
            }
            catch (e) {
                console.error(e);
                this.print(tr(`Ein Fehler ist aufgetreten. Seite neu laden oder \`${CMDNAMES.LOGIN}\` für einen neuen Versuch ...`));
            }
        }
        this.print(tr(`Tippe \`${CMDNAMES.HELP}\`, um Hilfe zu den verfügbaren Kommandos anzuzeigen.`));
        this.print(tr('Drücke Strg+C, um die Ausführung eines Kommandos abzubrechen.'));
        this.displayPrompt();
        let callback = response => {
            let tokenized = Game.parseCommandLine(response);
            if (tokenized === null) {
                this.displayPrompt();
                this.term.waitForCommand(callback);
                return;
            }
            if (typeof tokenized.command === 'undefined') {
                const c = Game.COMMANDS.map(cmd => {
                    return {
                        distance: levenshtein(cmd.name, tokenized.given_command),
                        name: cmd.name,
                    };
                });
                c.sort((a, b) => a.distance - b.distance);
                this.print(tr(`Unbekanntes Kommando: "${tokenized.given_command}"; meintest du \`${c[0].name}\`?`));
                this.print(tr(`Siehe \`${CMDNAMES.HELP}\` für eine Übersicht der verfügbaren Kommandos.`))
                this.displayPrompt();
                this.term.waitForCommand(callback);
                return;
            }
            this.showProgressbar();
            tokenized.command.fn.bind(this)(tokenized.params)
            .then(() => {
                this.print();
                this.displayPrompt();
                this.term.waitForCommand(callback);
            },
            e => {
                if (e) {
                    console.error(e);
                    this.print(`Fehler: ${e}`);
                }
                this.displayPrompt();
                this.term.waitForCommand(callback);
            });
        };
        this.term.waitForCommand(callback);
    }
    /**
     * @param {String} commandLine
     * @returns {object} object consisting of command, params and the given command
     */
    static parseCommandLine(commandLine) {
        let m = commandLine.match(RE.COMMANDLINE);
        if (m === null || m.length === 0) {
            return null;
        }
        const command = Game.COMMANDS.find(a => a.name === m[0]);
        return {
            command: command,
            params: m.slice(1),
            given_command: m[0],
        };
    }
    /**
     * return {User}
     */
    get user() {
        return this._user;
    }
    set user(user) {
        this._user = user;
        this.term.prompt = this.prompt;
        if (this.user === null) {
            this.login_state_el.textContent = tr(`nicht angemeldet`);
        }
        else {
            this.login_state_el.innerHTML = tr(`angemeldet als <i>${this.user.username}</i> (Punkte: <span id="score">${this.user.score}</span>, Level: <span id="level">${this.user.level}</span>)`);
        }
    }
    displayPrompt() {
        this.hideProgressbar();
        this.term.displayPrompt();
    }
    get prompt() {
        if (this.user !== null && this.user.in_room) {
            return tr(`<i>${this.user.username}</i>@<span title="${this.user.in_room.id.$oid}" class="room-name">${this.user.in_room.coords}</span>&gt; `);
        }
        return tr(`(nicht angemeldet)> `);
    }
    get term() {
        return this._term;
    }
    set term(term) {
        this._term = term;
    }
    loginDataPresent() {
        if (!this.online) {
            return false;
        }
        const jwt = localStorage.getItem('jwt');
        const parsedJWT = (jwt !== null && jwt !== 'null')
            ? User.parsedJWT(jwt)
            : null;
        return parsedJWT && parsedJWT.exp > currentUnixTimestamp();
    }
    async execute(command) {
        this.showProgressbar();
        await Game.COMMANDS.find(cmd => cmd.name === command).fn.bind(this)();
    }
    /**
     * Go through door.
     * @param {String} direction
     */
     async go(direction) {
        const url = constructURL(Game.URL.GO, {direction: direction});
        const response = await authenticatedRequest(url);
        const data = await response.json();
        return data;
    }
    /**
     * @param {String} direction - one of 'n', 'e', 's', 'w'
     * @returns {Boolean} - true if command succeeded, false otherwise
     */
    async stepThroughDoorway(direction) {
        this.showProgressbar();
        if (!this.loginDataPresent()) {
            this.print(tr(`Du musst angemeldet sein, um das Kommandos ausführen zu können.`));
            return false;
        }
        const neighbor = this.user.in_room.neighbors.find(neighbor => neighbor.direction === direction);
        if (!neighbor) {
            this.print(tr(`Du kannst nicht nach ${DIRECTIONS[direction]} gehen.`));
            return false;
        }
        const solved = this.user.solved.find(riddle => riddle.riddle_id.$oid === neighbor.riddle_id.$oid);
        if (solved) {
            const reply = await this.go(direction);
            if (reply.ok) {
                this.user.in_room = reply.room;
                this.term.prompt = this.prompt;
                if (this.user.in_room.exit) {
                    this.print(tr(`Du bist nach ${DIRECTIONS[direction]} gegangen, direkt in die <strong>FREIHEIT</strong>! Gratulation! Du hast den Ausgang des Labyrinths gefunden.`));
                }
                else {
                    this.print(tr(`Du bist nach ${DIRECTIONS[direction]} durch den Durchgang gegangen.`));
                    this.execute(CMDNAMES.WHEREAMI);
                }
                return true;
            }
            else {
                this.print(tr('Oh nein, irgendwas ist schiefgegangen. Diese Fehlermeldung hättest du eigentlich niemals sehen dürfen. Eigentlich ...'));
            }
        }
        else /* not solved yet! */ {
            this.print(tr(`Die Tür zum Durchgang ist verschlossen. Löse ein Rätsel, um sie zu öffnen:`));
            this.print(`- - - - - - - - - - - - - - - - -`);
            let riddle = await Riddle.loadByOID(neighbor.riddle_id.$oid);
            if (riddle.task) {
                this.print(parseMarkdown(riddle.task));
                this.term.container.appendChild(makeDownloadLinkMime(riddle.task, `task-${riddle.level}.md`, 'text/markdown'));
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
                            img.src = `data:${f.mimeType};base64,${f.data}`;
                            if (f.variants) {
                                img.srcset = f.variants.map(v => `data:${f.mimeType};base64,${v.data} ${v.scale}x`).join(' ');
                            }
                            img.style = `max-width: ${f.width}px;`;
                            this.term.container.appendChild(img);
                            this.term.container.appendChild(makeDownloadLink(f, f.data));
                            break;
                        case 'image/svg':
                            // fall-through
                        case 'image/svg+xml':
                            const svgimg = document.createElement('img');
                            svgimg.src = `data:image/svg+xml;base64,${f.data}`;
                            this.term.container.appendChild(svgimg);
                            this.term.container.appendChild(makeDownloadLink(f, f.data));
                            break;
                        case 'audio/mp3':
                            // fall-through
                        case 'audio/webm':
                            // fall-through
                        case 'audio/flac':
                            const audioUrl = `data:${f.mimeType};base64,${f.data}`;
                            this.print(`<audio autoplay controls><source src="${audioUrl}" type="${f.mimeType}"></audio>`);
                            break;
                        case 'text/plain':
                            // fall-through
                        case 'text/yaml':
                            this.term.container.appendChild(makeDownloadLink(f, f.data));
                            break;
                        case 'text/markdown':
                            this.print(parseMarkdown(Base64.decode(f.data)));
                            this.term.container.appendChild(makeDownloadLink(f, f.data));
                            break;
                        case 'text/html':
                            const embed = document.createElement('embed');
                            embed.src = `data:${f.mimeType};base64,${f.data}`;
                            embed.type = f.mimeType;
                            embed.style.width = "512px";
                            embed.style.height = "512px";
                            this.term.container.appendChild(embed);
                            this.term.container.appendChild(makeDownloadLink(f, f.data));
                            break;
                        case 'application/json':
                            // fall-through
                        case 'application/octet-stream':
                            // fall-through
                        case 'application/zip':
                            this.term.container.appendChild(makeDownloadLink(f, f.data));
                            break;
                        case 'application/pdf':
                            const pdf = document.createElement('embed');
                            pdf.src = `data:${f.mimeType};base64,${f.data}`;
                            pdf.type = f.mimeType;
                            pdf.style = 'width: 100%; background-color: white';
                            this.term.container.appendChild(pdf);
                            this.term.container.appendChild(makeDownloadLink(f, f.data));
                            break;
                        case 'video/webm':
                            // fall-through
                        case 'video/mp4':
                            // fall-through
                        case 'video/ogg':
                            const videoUrl = `data:${f.mimeType};base64,${f.data}`;
                            this.print(`<video autoplay controls><source src="${videoUrl}" type="${f.mimeType}"></video>`);
                            break;
                        default:
                            console.error(`Unknown file type: ${f.mimeType}`);
                    }
                }
            }
            this.print();
            if (riddle.credits) {
                this.print(tr(`<br/><b>Credits</b>: ${riddle.credits.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')}`));
            }
            this.print(`- - - - - - - - - - - - - - - - -`);
            this.print(tr(`Mit der richtigen Lösung des Rätsels öffnest du die Tür und bekommst ${riddle.difficulty} Punkt${riddle.difficulty === 1 ? '' : 'e'} gutgeschrieben.`));
            if (riddle.deduction > 0) {
                this.print(tr(`Achtung, für jede falsche Antwort ${riddle.deduction === 1 ? 'wird dir ein Punkt' : `werden dir ${riddle.deduction} Punkte`} abgezogen!`));
            }
            let reply = { solved: false };
            while (!reply.solved) {
                const solution = await this.getInput(tr('Deine Lösung? '));
                this.showProgressbar();
                if (solution.length > 0) {
                    reply = await riddle.solve(solution);
                    if (reply.solved) {
                        this.user.solved.push({riddle_id: {$oid: reply.riddle_id.$oid}});
                        const debriefing_response = await Riddle.getDebriefing(reply.riddle_id.$oid);
                        const debriefing = debriefing_response.debriefing;
                        this.print();
                        if (debriefing) {
                            this.print(parseMarkdown(debriefing));
                        }
                        else {
                            this.print(tr('Korrekt.'));
                        }
                        if (reply.level > this.user.level) {
                            this.print(tr(`\nDein Level ist von ${this.user.level} auf ${reply.level} gestiegen.`));
                            this.user.level = reply.level;
                            window.dispatchEvent(new CustomEvent('level', { detail: { level: reply.level }}));
                        }
                        this.print(tr('Die Tür ist nun offen.'));
                        await this.stepThroughDoorway(direction);
                    }
                    else {
                        this.print(tr('Leider falsch.'));
                        if (riddle.deduction > 0) {
                            this.print(tr(`Dir ${riddle.deduction === 1 ? 'wird ein Punkt' : `werden ${riddle.deduction} Punkte`} abgezogen.`));
                        }
                        this.print(tr('Versuchs noch einmal!'));
                    }
                    window.dispatchEvent(new CustomEvent('score', { detail: { score: reply.score }}));
                }
            }
            return true;
        }
        return false;
    }
    print(text) {
        this.term.writeln(text);
    }
    async chooseSecondFactor(factors) {
        const choice = parseInt(await this.getInput(tr(`1..${factors.length}? `), { match: Array.from(factors.keys()).map(i => i+1) }));
        return factors[choice-1];
    }
    async loginTOTP(username, totp) {
        console.debug(`loginTOTP("${username}", "${totp}")`);
        const response = await fetch(Game.URL.USER.TOTP.LOGIN, {
            method: 'POST',
            cache: 'no-cache',
            mode: 'cors',
            credentials: 'same-origin',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                username: username,
                totp: totp,
            }),
        });
        const reply = await response.json();
        switch (response.status) {
            case 200:
                if (reply.ok) {
                    this.user = new User(reply);
                }
                else {
                    this.print(tr(`Die Anmeldung ist fehlgeschlagen. Meldung vom Server: "${totp_reply.message}".`));
                    return Promise.reject();
                }
                break;
            default:
                this.print(tr(`Die Anmeldung ist fehlgeschlagen: ${totp_reply.message}.`));
                return Promise.reject();
        }
    }
    async loginFIDO2(username) {
        const reply = await fetch(constructURL(Game.URL.USER.WEBAUTHN.LOGIN.START, { username: username }), {
            method: 'POST',
            cache: 'no-cache',
            mode: 'cors',
            credentials: 'same-origin',
            headers: {
                'Content-Type': 'application/json',
            },
        })
        .then(response => response.json());
        if (!reply.ok) {
            this.print(tr(`Fehler bei der Authentifizierungsanfrage. Noch mal versuchen mit \`${CMDNAMES.LOGIN}\` ...`));
            return Promise.reject();
        }
        let rcr = reply.rcr;
        rcr.publicKey.challenge = Base64.toArray(rcr.publicKey.challenge.replaceAll('_', '/').replaceAll('-', '+'));
        rcr.publicKey.allowCredentials.forEach(listItem => listItem.id = Base64.toArray(listItem.id.replaceAll('_', '/').replaceAll('-', '+')));
        const assertion = await navigator.credentials.get({
            publicKey: rcr.publicKey
        });
        const authData = assertion.response.authenticatorData;
        const clientDataJSON = assertion.response.clientDataJSON;
        const rawId = assertion.rawId;
        const sig = assertion.response.signature;
        const userHandle = assertion.response.userHandle;
        const finishData = {
            id: assertion.id,
            rawId: arrayToBase64(rawId),
            type: assertion.type,
            response: {
                authenticatorData: arrayToBase64(authData),
                clientDataJSON: arrayToBase64(clientDataJSON),
                signature: arrayToBase64(sig),
                userHandle: arrayToBase64(userHandle),
            },
        };
        const finish_reply = await fetch(constructURL(Game.URL.USER.WEBAUTHN.LOGIN.FINISH, {username: username}), {
            method: 'POST',
            cache: 'no-cache',
            mode: 'cors',
            credentials: 'same-origin',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(finishData),
        })
        .then(response => response.json());
        if (!finish_reply.ok) {
            this.print(tr(`Fehler bei, Abschluss der Authentifizierung. `));
            return Promise.reject();
        }
        this.user = new User(finish_reply);
        return Promise.resolve();
    }
    async enableFIDO2(username) {
        console.debug(`enableFIDO2("${username}")`);
        const reply = await authenticatedRequest(Game.URL.USER.WEBAUTHN.REGISTER.START, 'POST')
        .then(response => response.json())
        .then(data => {
            if (data.ok) {
                let ccr = data.ccr;
                try {
                    const url_unsafe_ccr = ccr.publicKey.challenge.replaceAll('_', '/').replaceAll('-', '+');
                    ccr.publicKey.challenge = Base64.toArray(url_unsafe_ccr);
                }
                catch (e) {
                    console.error(e);
                    this.print(tr(`Fehler: ${e}`));
                }
                try {
                    const url_unsafe_id = ccr.publicKey.user.id.replaceAll('_', '/').replaceAll('-', '+');
                    ccr.publicKey.user.id = Base64.toArray(url_unsafe_id);
                }
                catch (e) {
                    console.error(e);
                    this.print(tr(`Fehler: ${e}`));
                }
                if (ccr.publicKey.excludeCredentials) {
                    for (let i = 0; i < ccr.publicKey.excludeCredentials.length; ++i) {
                        const url_unsafe_cred = ccr.publicKey.excludeCredentials[i].id.replaceAll('_', '/').replaceAll('-', '+');
                        ccr.publicKey.excludeCredentials[i].id = Base64.toArray(url_unsafe_cred);
                    }
                }
                return navigator.credentials.create({
                    publicKey: ccr.publicKey
                });
            }
            else {
                console.error(data.message);
                this.print(tr(`Fehler: ${data.message}`));
            }
        })
        .then(credential => {
            const attestationObject = credential.response.attestationObject;
            const clientDataJSON = credential.response.clientDataJSON;
            const rawId = credential.rawId;
            const data = {
                id: credential.id,
                rawId: arrayToBase64(rawId),
                type: credential.type,
                response: {
                    attestationObject: arrayToBase64(attestationObject),
                    clientDataJSON: arrayToBase64(clientDataJSON),
                },
            };
            const finish_response = authenticatedRequest(Game.URL.USER.WEBAUTHN.REGISTER.FINISH, 'POST', JSON.stringify(data))
            .then(response => response.json())
            .catch(e => console.error(e));
            return finish_response;
        })
        .catch(e => console.error(e));
        return reply;
    }
    async proceedWith2FA(userData) {
        this.user = new User(userData);
        this.print(tr(`Die Aktivierung war erfolgreich. Dein Konto wurde unter dem Namen <i>${userData.username}</i> angelegt.`));
        if (userData.recovery_keys) {
            this.print(tr(`Hier sind deine ${userData.recovery_keys.length} Wiederherstellungsschlüssel für den Fall, dass du dein Passwort vergisst:`));
            this.print();
            for (const key of userData.recovery_keys) {
                this.print(`    ${key}`);
            }
            this.print(tr('\nVerwahre sie bitte sicher! Sie werden dir hier das erste und letzte Mal angezeigt.'));
        }
        if (userData.totp) {
            this.print(tr('\nUnd hier ist dein zweiter Faktor als QR-Code für TOTP-Generatoren wie <a href="https://authy.com/" targer="_blank">Authy</a> oder <a href="https://de.wikipedia.org/wiki/Google_Authenticator" target="_blank">Google Authenticator</a>:'));
            this.print();
            if (userData.totp.qrcode) {
                const img = document.createElement('img');
                img.src = `data:image/png;base64,${userData.totp.qrcode}`;
                img.style.width = '256px';
                img.style.height = '256px';
                img.style.display = 'block';
                this.term.container.appendChild(img);
            }
            this.print(tr('\nDu brauchst den zweiten Faktor für spätere Logins.'));
            this.print(tr(`Du kannst dir das TOTP-Geheimnis auch notieren: ${userData.totp.secret}`));
        }
        else {
            this.print(tr('Du hast als zweiten Faktor zur Authentifizierung FIDO2 ausgewählt. Bitte wähle aus dem Browser-Dialog das Token dafür.'))
            const reply = await this.enableFIDO2(userData.username);
            if (reply.ok && reply.jwt) {
                this.user = new User(reply);
            }
        }
    }
    logout() {
        this.user = null;
        localStorage.removeItem('jwt');
    }
    async whoami() {
        try {
            const response = await authenticatedRequest(Game.URL.USER.WHOAMI);
            const reply = await response.json();
            if (response.status !== 200) {
                this.print(tr(`Zu Hülf! Etwas ist schiefgegangen.`));
            }
            return reply;
        }
        catch (e) {
            console.error(e);
            this.print(tr('Der whoami-Service steht derzeit nicht zur Verfügung. Versuchs bitte später noch einmal ...'));
        }
    }
    /**
     * @param {String} username
     * @param {String} pin
     */
    async activate(username, pin) {
        let data = {
            username: username,
            pin: pin | 0,
        };
        let user;
        try {
            const response = await fetch(Game.URL.USER.ACTIVATE, {
                method: 'POST',
                cache: 'no-cache',
                mode: 'cors',
                credentials: 'same-origin',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(data)
            });
            switch (response.status) {
                case 200:
                    user = await response.json();
                    break;
                case 400:
                    this.print(tr(`Aktivierung fehlgeschlagen! Entweder existiert das Konto nicht oder es ist bereits aktiviert.`));
                    break;
                case 403:
                    this.print(tr(`Aktivierung fehlgeschlagen! Hast du die richtige PIN eingegeben?`));
                    break;
                default:
                    this.print(tr(`Aktivierung aufgrund eines unbekannten Fehlers fehlgeschlagen! `));
                    break;
            }
        }
        catch (e) {
            console.error(e);
            this.print(tr('Der Aktivierungs-Service steht derzeit nicht zur Verfügung. Versuchs bitte später noch einmal ...'));
        }
        return user;
    }
}

let main = () => {
    if ('customElements' in window) {
        customElements.define('game-div', Game);
        customElements.define('terminal-div', Terminal);
        customElements.define('progress-bar', ProgressBar);
        const game = document.querySelector('game-div');
        game.classList.remove('hidden');
        game.term.setCommands(Game.COMMANDS.map(cmd => cmd.name));
        game.play();
    }
    else {
        document.querySelector('#so-sad').classList.remove('hidden');
    }
};

console.log('%c Labyrinth %c (revision %s) - Löse Rätsel, finde den Weg heraus.\nCopyright © 2022 Oliver Lau <oliver@ersatzworld.net>\nAlle Rechte vorbehalten.',
    'background: #111; color: #24cdd3; font-weight: bold;',
    'background: transparent; color: #222; font-weight: normal;',
    REVISION)

window.addEventListener('load', main);