class Terminal extends HTMLElement {
    constructor() {
        super();
        this._prompt = '> ';
        this.textarea = null;
        this._commands = [];
        this.historyMax = 100;
        this.loadHistory();
        const shadowRoot = this.attachShadow({ mode: 'open' });
        this.el = document.createElement('div');
        this.el.className = 'terminal scanlines';
        const style = document.createElement('style');
        style.textContent = `
.terminal {
    color: #24cdd3;
    font-size: 10pt;
    font-family: 'Victor Mono', monospace;
    line-height: 14pt;
    position: absolute;
    width: calc(100% - 4px);
    height: calc(100% - 40px);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    word-break: break-all;
}
@keyframes blink {
    50% {
        background-color: transparent;
        color: #090b13;
    }
}
.terminal .cursor {
    display: inline-block;
    animation-duration: 750ms;
    animation-name: blink;
    animation-iteration-count: infinite;
    animation-direction: normal;
    animation-timing-function: steps(1);
    position: absolute;
    background-color: #fff;
    color: #090b13;
    width: 8px;
    height: 17px;
}
.terminal.blurred .cursor {
    outline: 1px solid #fff;
    outline-style: inset -1px;
    animation-play-state: paused;
    background-color: transparent;
}
.spoiler {
    color: transparent;
    background-color: #24cdd3;
    transition-duration: 3117ms;
    transition-timing-function: ease-out;
    transition-property: color, background-color;
    cursor: pointer;
}
.spoiler:active {
    color: inherit;
    background-color: inherit;
}
div.row {
    display: inline-block;
    position: relative;
}
span {
    display: inline-block;
    position: relative;
}
a, a:visited {
    color: #24cdd3;
    text-decoration: none;
    border-bottom: 2px dotted #24cdd3
} 
svg, img, embed {
    border: 4px solid #24cdd3;
    padding: 4px;
    width: calc(100% - 16px);
    box-shadow: 0 0 6px #2fa8b86e;
}
.suggestion {
    opacity: 0.7;
}
.terminal textarea {
    position: absolute;
    opacity: 0;
    left: -9999px;
    width: 0;
    height: 0;
    z-index: -10;
}
.b400 {
    font-weight: 400;
}
.b500 {
    font-weight: 500;
}
.b600 {
    font-weight: 600;
}
.b700 {
    font-weight: 700;
}
.scanlines {
    mix-blend-mode: normal;
}
.scanlines::before {
  content: "";
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  right: 0;
  z-index: -10;
  background: repeating-linear-gradient(
    to bottom,
    transparent 0%,
    rgba(255, 255, 255, 0.05) .15%,
    transparent .3%
  );
}
.clickable-direction {
    cursor: pointer;
    padding: 0 5pt 0 5pt;
    border: 0.5px dotted #24cdd3;
}
.clickable-direction:active {
    background-color: #24cdd3;
    color: #090b13;
}
`;
        shadowRoot.appendChild(style);
        shadowRoot.appendChild(this.el);
        this.boundSaveHistory = function() {
            this.saveHistory();
        }.bind(this);
        this.boundFocus = function(e) {
            e.preventDefault();
            e.stopPropagation();
            this.focus();
        };
        this.boundBlur = function() {
            this.blur();
        };
    }
    connectedCallback() {
        window.addEventListener('unload', this.boundSaveHistory);
    }
    disconnectedCallback() {
        window.removeEventListener('unload', this.boundSaveHistory);
    }
    attributeChangedCallback(attrName, oldVal, newVal) {
    }
    get container() {
        return this.el;
    }
    get prompt() {
        return this._prompt;
    }
    /**
     * @param {String[]} prompt
     */
    set prompt(prompt) {
        this._prompt = prompt;
    }
    get commands() {
        return this._commands;
    }
    /**
     * @param {String[]} commands
     */
    setCommands(commands) {
        this._commands = commands;
    }
    trimHistory() {
        if (this.history.length > this.historyMax) {
            this.history = this.history.slice(-this.historyMax);
        }
        this.historyIndex = this.history.length;
    }
    /**
     * @param {String} cmd
     */
    addToHistory(cmd) {
        if (cmd.length > 0) {
            this.history.push(cmd);
        }
        this.trimHistory();
    }
    clearHistory() {
        this.history = [];
        this.historyIndex = 0;
    }
    saveHistory() {
        localStorage.setItem('history', JSON.stringify(this.history));
    }
    loadHistory() {
        this.history = JSON.parse(localStorage.getItem('history') || '[]');
        if (!(this.history instanceof Array)) {
            this.history = [];
        }
        this.trimHistory();
        this.historyIndex = this.history.length;
    }
    get currentElement() {
        return this._span;
    }
    /**
     * @param {String} text
     */
    write(text) {
        this._span = document.createElement('span');
        this._span.innerHTML = text || '';
        this.el.appendChild(this._span);
        return this._span;
    }
    /**
     * @param {String} text
     */
    writeln(text) {
        const span = this.write(text);
        this.el.appendChild(document.createElement('br'));
        return span;
    }
    appendNode(node) {
        this.el.appendChild(node);
    }
    displayPrompt() {
        this.write(this.prompt);
    }
    enter(text) {
        this.textarea.value = text;
        this.textarea.dispatchEvent(new Event('input'));
        this.textarea.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter'}));
    }
    /**
     * @param {{}} param
     */
    async waitForInput(param = {}) {
        return new Promise(function(resolve, reject) {
            let div = document.createElement('div');
            div.className = 'row';
            let cmdSpan = document.createElement('span');
            cmdSpan.className = 'input';
            let tailSpan = document.createElement('span');
            tailSpan.className = 'tail';
            let textarea = document.createElement('textarea');
            textarea.setAttribute('autocorrect', 'off');
            textarea.setAttribute('autocapitalize', 'off');
            textarea.setAttribute('spellcheck', 'off');
            textarea.setAttribute('tabindex', 0);
            textarea.className = 'input-helper';
            this.el.appendChild(textarea);
            let cursor = document.createElement('span');
            cursor.className = 'cursor';
            cursor.innerHTML = '&nbsp;';
            let suggSpan = document.createElement('span');
            suggSpan.className = 'suggestion';
            div.appendChild(cmdSpan);
            div.appendChild(cursor);
            div.appendChild(tailSpan);
            this.el.appendChild(div);
            setTimeout(function() { window.scrollTo(0, this.scrollHeight); }.bind(this), 100);
            const onFocus = function(e) {
                e.preventDefault();
                e.stopImmediatePropagation();
                textarea.focus();
                this.el.classList.remove('blurred');
                return true;
            }.bind(this);
            const onBlur = function(e) {
                this.el.classList.add('blurred');
            }.bind(this);
            const onMouseup = e => {
                textarea.focus();
                this.el.classList.remove('blurred');
                e.stopPropagation();
                e.preventDefault();
            };
            const updateTextarea = () => {
                const value = param.password ? '•'.repeat(textarea.value.length) : textarea.value;
                if (textarea.selectionStart < value.length) {
                    cmdSpan.textContent = value.substring(0, textarea.selectionStart);
                    cursor.textContent = value.substring(textarea.selectionStart, this.textarea.selectionStart+1);
                    tailSpan.textContent = value.substring(textarea.selectionStart);
                }
                else {
                    cmdSpan.textContent = value;
                    cursor.innerHTML = '';
                    tailSpan.textContent = '';
                }    
            };
            const onInput = e => {
                updateTextarea();
            };
            const onKeydown = e => {
                switch (e.key) {
                    case 'c':
                        if (e.ctrlKey) {
                            this.writeln(tr('^C'));
                            textarea.removeEventListener('keyup', onKeyup);
                            textarea.removeEventListener('keydown', onKeydown);
                            textarea.removeEventListener('input', onInput);
                            window.removeEventListener('focus', onFocus);
                            window.removeEventListener('blur', onBlur);
                            cmdSpan.classList.remove('input');
                            cursor.remove();
                            tailSpan.remove();
                            e.stopPropagation();
                            textarea.remove();
                            e.preventDefault();
                            reject();
                        }
                        break;
                    case 'Enter':
                    case 'Escape':
                        textarea.removeEventListener('keyup', onKeyup);
                        textarea.removeEventListener('keydown', onKeydown);
                        textarea.removeEventListener('input', onInput);
                        window.removeEventListener('focus', onFocus);
                        window.removeEventListener('blur', onBlur);
                        cmdSpan.classList.remove('input');
                        cursor.remove();
                        tailSpan.remove();
                        if (e.key === 'Enter') {
                            resolve(textarea.value);
                        }
                        else {
                            resolve();
                        }
                        textarea.remove();
                        e.stopPropagation();
                        e.preventDefault();
                        break;
                    default:
                        break;
                }
            }
            const onKeyup = e => {
                switch (e.key) {
                    case 'ArrowLeft':
                        // fall-through
                    case 'ArrowRight':
                        updateTextarea();
                        break;
                    default:
                        break;
                    }
            };
            textarea.addEventListener('input', onInput);
            textarea.addEventListener('keyup', onKeyup);
            textarea.addEventListener('keydown', onKeydown);
            textarea.focus();
            window.addEventListener('focus', onFocus);
            window.addEventListener('blur', onBlur);
            window.addEventListener('mouseup', onMouseup);
        }.bind(this));
    }
    /**
     * @param {Function} callback
     */
    async waitForCommand(callback) {
        let div = document.createElement('div');
        div.className = 'row';
        let cmdSpan = document.createElement('span');
        cmdSpan.className = 'input';
        let tailSpan = document.createElement('span');
        tailSpan.className = 'tail';
        this.textarea = document.createElement('textarea');
        this.textarea.setAttribute('autocorrect', 'off');
        this.textarea.setAttribute('autocapitalize', 'off');
        this.textarea.setAttribute('spellcheck', 'off');
        this.textarea.setAttribute('tabindex', 0);
        this.textarea.className = 'input-helper';
        this.el.appendChild(this.textarea);
        let cursor = document.createElement('span');
        cursor.className = 'cursor';
        let suggSpan = document.createElement('span');
        suggSpan.className = 'suggestion';
        div.appendChild(cmdSpan);
        div.appendChild(cursor);
        div.appendChild(tailSpan);
        div.appendChild(suggSpan);
        this.el.appendChild(div);
        let suggestion;
        const onFocus = function(e) {
            e.preventDefault();
            e.stopImmediatePropagation();
            this.textarea.focus();
            this.el.classList.remove('blurred');
            return true;
        }.bind(this);
        const onBlur = function(e) {
            this.el.classList.add('blurred');
        }.bind(this);
        let onMouseup = e => {
            this.textarea.focus();
            this.el.classList.remove('blurred');
            e.stopPropagation();
            e.preventDefault();
        };
        const updateTextarea = () => {
            if (this.textarea.selectionStart < this.textarea.value.length) {
                cmdSpan.textContent = this.textarea.value.substring(0, this.textarea.selectionStart);
                cursor.textContent = this.textarea.value.substring(this.textarea.selectionStart, this.textarea.selectionStart+1);
                tailSpan.textContent = this.textarea.value.substring(this.textarea.selectionStart);
            }
            else {
                cmdSpan.textContent = this.textarea.value;
                cursor.innerHTML = '&nbsp;';
                tailSpan.textContent = '';
            }
        };
        const onInput = e => {
            updateTextarea();
        };
        const onKeydown = e => {
            switch (e.key) {
                case 'Enter':
                    this.textarea.removeEventListener('keyup', onKeyup);
                    this.textarea.removeEventListener('keydown', onKeydown);
                    this.textarea.removeEventListener('input', onInput);
                    window.removeEventListener('focus', onFocus);
                    window.removeEventListener('blur', onBlur);
                    cmdSpan.classList.remove('input');
                    cursor.remove();
                    this.writeln();
                    this.addToHistory(this.textarea.value);
                    suggSpan.remove();
                    callback(this.textarea.value);
                    e.stopPropagation();
                    return e.preventDefault();
                case 'Tab':
                    if (suggestion && suggestion.length > 0) {
                        this.textarea.value = suggestion;
                        cmdSpan.textContent = suggestion;
                        suggSpan.textContent = '';
                    }
                    e.stopPropagation();
                    e.preventDefault();
                    break;
                default:
                    if (!e.ctrlKey && !e.metaKey && !e.altKey && !e.cmdKey) {
                        const fragment = this.textarea.value + e.key;
                        if (fragment.length > 0) {
                            suggestion = this.commands.find(c => c.startsWith(fragment));
                            suggSpan.textContent = suggestion ? suggestion.substring(fragment.length) : '';
                        }
                    }
                    break;
            }
        }
        const onKeyup = e => {
            switch (e.key) {
                case 'ArrowUp':
                    if (this.historyIndex > 0) {
                        --this.historyIndex;
                        this.textarea.value = this.history[this.historyIndex];
                        cmdSpan.textContent = this.history[this.historyIndex];
                    }
                    return e.preventDefault();
                case 'ArrowDown':
                    if (this.historyIndex < this.history.length-1) {
                        ++this.historyIndex;
                        this.textarea.value = this.history[this.historyIndex];
                        cmdSpan.textContent = this.history[this.historyIndex];
                    }
                    else {
                        cmdSpan.textContent = '';
                        tailSpan.textContent = '';
                    }
                    return e.preventDefault();
                case 'ArrowLeft':
                    // fall-through
                case 'ArrowRight':
                    updateTextarea();
                    break;
                default:
                    break;
                }
        };
        this.textarea.addEventListener('input', onInput);
        this.textarea.addEventListener('keyup', onKeyup);
        this.textarea.addEventListener('keydown', onKeydown);
        this.textarea.focus();
        window.addEventListener('focus', onFocus);
        window.addEventListener('blur', onBlur);
        window.addEventListener('mouseup', onMouseup);
    }
    clear() {
        this.el.textContent = '';
    }
}
