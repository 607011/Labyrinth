class User {
    constructor(userData) {
        this._parsedJWT = null;
        this.update(userData);
    }
    update(userData) {
        for (const [key, value] of Object.entries(userData)) {
            this[key] = value;
        }
        localStorage.setItem('jwt', this.jwt);
        if (this.jwt !== null) {
            this._parsedJWT = User.parsedJWT(this.jwt);
        }
    }
    toString() {
        return this.username;
    }
    /**
     * @param {String} direction - a JSON Web Token
     * @returns {String} the parsed token
     */
    static parsedJWT(token) {
        const base64Url = token.split('.')[1];
        const base64 = base64Url.replaceAll('-', '+').replaceAll('_', '/');
        const jsonPayload = decodeURIComponent(atob(base64).split('').map(function(c) {
            return '%' + ('00' + c.charCodeAt(0).toString(16)).slice(-2);
        }).join(''));
        let jwt = JSON.parse(jsonPayload);
        if (jwt.iat) {
            jwt.iat = jwt.iat | 0;
        }
        if (jwt.exp) {
            jwt.exp = jwt.exp | 0;
        }
        return jwt;
    }
}
