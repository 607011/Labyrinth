/**
 * @returns current UNIX timestamp in seconds as integer
 */
 const currentUnixTimestamp = () => {
    return (Date.now() / 1000) | 0;
}

/**
 * @param {String} template - the URL template, e.g. "/user/get/by/:userid"
 * @param {object} params - an object: the values of the properties replace the placeholders in the URL with the same name as the properties'
 * @returns {String} - the constructed URL
 */
const constructURL = (template, params) => {
    let url = template;
    for (const [key, value] of Object.entries(params)) {
        const re = new RegExp(`:${key}`);
        url = url.replace(re, value);
    }
    return url;
};

/**
 * Construct a HTML element that, when clicked, will initiate a download of a file.
 * 
 * @param file {object} - object containing information about the file
 * @returns {HTMLSpanElement} the constructed HTML element
 */
const makeDownloadLink = (file) => {
    let span = document.createElement('div');
    span.textContent = '-> ';
    let a = document.createElement('a');
    a.download = file.originalName;
    a.title = `Download image as ${file.originalName}`;
    a.href = `${UPLOAD_FOLDER}/${file.uploadedName}`;
    a.textContent = `Download ${file.originalName}`;
    span.appendChild(a);
    return span;
};

/**
 * Construct a HTML element that, when clicked, will initiate a download of a file.
 * 
 * @returns {HTMLSpanElement} the constructed HTML element
 */
 const makeDownloadLinkMime = (text, filename, mimetype) => {
    let span = document.createElement('div');
    span.textContent = '-> ';
    let a = document.createElement('a');
    a.download = filename;
    a.title = 'Download task as file';
    a.href = `data:${mimetype};base64,${Base64.encode(text)}`;
    a.textContent = `Download task as file '${filename}'`;
    span.appendChild(a);
    return span;
};

class Base64 {
    static encode(string) {
        const codeUnits = new Uint16Array(string.length);
        for (let i = 0; i < codeUnits.length; i++) {
          codeUnits[i] = string.charCodeAt(i);
        }
        return btoa(String.fromCharCode(...new Uint8Array(codeUnits.buffer)));
    }
    static decode(encoded) {
        const bytes = Base64.toArray(encoded);
        return String.fromCharCode(...new Uint8Array(bytes.buffer));
    }
    static toArray(encoded) {
        const binary = atob(encoded);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < bytes.length; i++) {
          bytes[i] = binary.charCodeAt(i);
        }
        return bytes;
    }
}

const arrayToBase64 = value => {
    return btoa(String.fromCharCode.apply(null, new Uint8Array(value)))
        .replace(/\+/g, "-")
        .replace(/\//g, "_")
        .replace(/=/g, "");;
}

/**
 * @param {String} text
 */
const parseMarkdown = markdown => {
    const html = markdown
        .replace(/^### (.*$)/gim, '<span class="b500">$1</span>')
        .replace(/^## (.*$)/gim, '<span class="b600">$1</span>')
        .replace(/^# (.*$)/gim, '<span class="b700">$1</span>')
        .replace(/^> /gim, '    ')
        .replace(/\*\*(.*?)\*\*/gim, '<strong>$1</strong>')
        .replace(/@@@(.*?)@@@/gim, '<span class="spoiler">$1</span>')
        .replace(/\*(.*?)\*/gim, '<i>$1</i>')
        .replace(/!\[(.*?)\]\((.*?)\)/gim, '<img alt="1" src="$2" />')
        .replace(/\[(.*?)\]\((.*?)\)/gim, '<a href="$2" target="blank">$1</a>')
        .replace(/\n/gim, '<br/>')
        .replace(/```/gim, '');
    return html.trim();
}


/**
 * Calculate the Levenshtein distance between two words.
 * The bigger the distance, the less similar the words.
 * @returns {Integer} Levenshtein distance
 */
const levenshtein = (function () {
    const _min = (d0, d1, d2, bx, ay) => {
        return d0 < d1 || d2 < d1
            ? d0 > d2
                ? d2 + 1
                : d0 + 1
            : bx === ay
                ? d1
                : d1 + 1;
    }
    return function(a, b) {
        if (a === b) {
            return 0;
        }
        if (a.length > b.length) {
            [a, b] = [b, a];
        }
        let la = a.length;
        let lb = b.length;
        while (la > 0 && (a.charCodeAt(la - 1) === b.charCodeAt(lb - 1))) {
            --la;
            --lb;
        }
        let offset = 0;
        while (offset < la && (a.charCodeAt(offset) === b.charCodeAt(offset))) {
            ++offset;
        }
        la -= offset;
        lb -= offset;
        if (la === 0 || lb < 3) {
            return lb;
        }
        let x = 0, d0, d1, d2, d3, dd, dy, ay, bx0, bx1, bx2, bx3;
        let vector = [];
        for (let y = 0; y < la; y++) {
            vector.push(y + 1);
            vector.push(a.charCodeAt(offset + y));
        }
        const len = vector.length - 1;
        for (; x < lb - 3;) {
            bx0 = b.charCodeAt(offset + (d0 = x));
            bx1 = b.charCodeAt(offset + (d1 = x + 1));
            bx2 = b.charCodeAt(offset + (d2 = x + 2));
            bx3 = b.charCodeAt(offset + (d3 = x + 3));
            dd = (x += 4);
            for (let y = 0; y < len; y += 2) {
                dy = vector[y];
                ay = vector[y + 1];
                d0 = _min(dy, d0, d1, bx0, ay);
                d1 = _min(d0, d1, d2, bx1, ay);
                d2 = _min(d1, d2, d3, bx2, ay);
                dd = _min(d2, d3, dd, bx3, ay);
                vector[y] = dd;
                d3 = d2;
                d2 = d1;
                d1 = d0;
                d0 = dy;
            }
        }
        for (; x < lb;) {
            bx0 = b.charCodeAt(offset + (d0 = x));
            dd = ++x;
            for (let y = 0; y < len; y += 2) {
                dy = vector[y];
                vector[y] = dd = _min(dy, d0, dd, bx0, vector[y + 1]);
                d0 = dy;
            }
        }
        return dd;
    };
})();


/** Construct a fetch Promise with an authorization header containing a JSON Web Token.
 * @returns {Promise}
 */
const authenticatedRequest = async (url, method='GET', data=null) => {
    switch (method) {
        case 'GET':
            return fetch(url, {
                method: method,
                cache: 'no-cache',
                mode: 'cors',
                credentials: 'same-origin',
                headers: {
                    'Authorization': `Bearer ${localStorage.getItem('jwt')}`,
                },
            });
        default:
            const body = (data === null)
            ? undefined
            : JSON.stringify(data);
            return fetch(url, {
                method: method,
                cache: 'no-cache',
                mode: 'cors',
                credentials: 'same-origin',
                headers: {
                    'Authorization': `Bearer ${localStorage.getItem('jwt')}`,
                    'Content-Type': 'application/json',
                    },
                body: body,
            });
    }
};
