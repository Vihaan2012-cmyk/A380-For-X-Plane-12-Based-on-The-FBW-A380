import gselib as g


def grooved(profile, grooves=3, depth=16.0, half=11.0):
    rmax = max(r for r, _ in profile)
    out = []
    for i, (r, z) in enumerate(profile):
        out.append((r, z))
        if i + 1 < len(profile) and r == rmax and profile[i + 1][0] == rmax:
            z0, z1 = z, profile[i + 1][1]
            for j in range(1, grooves + 1):
                zc = z0 + (z1 - z0) * j / (grooves + 1)
                sgn = 1.0 if z1 > z0 else -1.0
                out += [(rmax, zc - sgn * half), (rmax - depth, zc - sgn * (half - 4)), (rmax - depth, zc + sgn * (half - 4)), (rmax, zc + sgn * half)]
    return out


def tyre(f, name, center, axis, profile, mat, c, segments=72):
    return g.lathe(f, name, center, axis, grooved(profile), mat, c, segments=segments)
