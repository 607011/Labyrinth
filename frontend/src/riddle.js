class Riddle {
    static URL = {
        INFO: `${HOST}/admin/riddle/by/level/:level`,
        LOAD: `${HOST}/riddle/:oid`,
        SOLVE: `${HOST}/riddle/solve/:oid`,
        DEBRIEFING: `${HOST}/riddle/debriefing/:oid`,
    };
    static async loadByLevel(level) {
        const url = constructURL(Riddle.URL.INFO, {level});
        const response = await authenticatedRequest(url);
        const data = await response.json();
        let riddle = new Riddle(data);
        return riddle;
    }
    /**
     * @param {String} ObjectId - the ObjectId of the riddle to load
     * @returns {Riddle} - Riddle object
     */
    static async loadByOID(oid) {
        const url = constructURL(Riddle.URL.LOAD, {oid});
        const response = await authenticatedRequest(url);
        const data = await response.json();
        let riddle = new Riddle(data);
        return riddle;
    }
    static async getDebriefing(oid) {
        const url = constructURL(Riddle.URL.DEBRIEFING, {oid});
        const response = await authenticatedRequest(url);
        const data = await response.json();
        return data;
    }
    /**
     * @param {String} solution - the solution of the riddle
     * @returns {Object} {riddle_id: ObjectId, solved: bool, level: u32, message: Option<String>}
     */
    async solve(solution) {
        const url = constructURL(Riddle.URL.SOLVE, {oid: this.id.$oid});
        const response = await authenticatedRequest(url, 'POST', {solution});
        const data = await response.json();
        return data;
    }
    /**
     * @constructor
     * @param {object} data 
     */
    constructor(data) {
        for (const [key, value] of Object.entries(data)) {
            this[key] = value;
        }
    }
    get id() {
        return this._id;
    }
    set id(id) {
        this._id = id;
    }
    toString() {
        return `Riddle#${this.id}`;
    }
}
