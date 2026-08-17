function d(s) {
    var a, i, j, c, c0, c1, c2, r;
    var _s = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_$';
    var _r64 = function(s, b) {
        return ((s | (s << 6)) >>> (b % 6)) & 63;
    };
    a = [];
    c = [];
    for (i = 0; i < s.length; i++) {
        c0 = s.charCodeAt(i);
        if (c0 & ~255) {
            c0 = (c0 >> 8) ^ c0;
        }
        c.push(c0);
        if (c.length == 3 || i == s.length - 1) {
            while (c.length < 3) {
                c.push(0);
            }
            a.push((c[0] >> 2) & 63);
            a.push(((c[1] >> 4) | (c[0] << 6)) & 63);
            a.push(((c[1] << 4) | (c[2] >> 2)) & 63);
            a.push(c[2] & 63);
            c = [];
        }
    }
    while (a.length < 16) {
        a.push(0);
    }
    r = 0;
    for (i = 0; i < a.length; i++) {
        r ^= (_r64(a[i] ^ (r | i), i) ^ _r64(i, r)) & 63;
    }
    for (i = 0; i < a.length; i++) {
        a[i] = (_r64((r | i & a[i]), r) ^ a[i]) & 63;
        r += a[i];
    }
    for (i = 16; i < a.length; i++) {
        a[i % 16] ^= (a[i] + (i >>> 4)) & 63;
    }
    for (i = 0; i < 16; i++) {
        a[i] = _s.substr(a[i], 1);
    }
    a = a.slice(0, 16).join('');
    return a;
}
