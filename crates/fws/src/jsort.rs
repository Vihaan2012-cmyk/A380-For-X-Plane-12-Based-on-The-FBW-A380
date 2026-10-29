//! `Array.prototype.sort` as QuickJS runs it (the engine the FWS goldens were recorded with): its `rqsort` with ties
//! broken by the original position, so comparators that never return 0 give the same order as in the recording.

fn med3<E>(a: &[E], x: usize, y: usize, z: usize, cmp: &mut impl FnMut(&E, &E) -> i32) -> usize {
    if cmp(&a[x], &a[y]) < 0 {
        if cmp(&a[y], &a[z]) < 0 {
            y
        } else if cmp(&a[x], &a[z]) < 0 {
            z
        } else {
            x
        }
    } else if cmp(&a[y], &a[z]) > 0 {
        y
    } else if cmp(&a[x], &a[z]) < 0 {
        x
    } else {
        z
    }
}

fn sift<E>(a: &mut [E], mut r: usize, end: usize, cmp: &mut impl FnMut(&E, &E) -> i32) {
    loop {
        let mut c = r * 2 + 1;
        if c >= end {
            break;
        }
        if c < end - 1 && cmp(&a[c], &a[c + 1]) <= 0 {
            c += 1;
        }
        if cmp(&a[r], &a[c]) > 0 {
            break;
        }
        a.swap(r, c);
        r = c;
    }
}

fn heapsort<E>(a: &mut [E], cmp: &mut impl FnMut(&E, &E) -> i32) {
    let n = a.len();
    if n < 2 {
        return;
    }
    let mut i = n / 2;
    while i > 0 {
        i -= 1;
        sift(a, i, n, cmp);
    }
    let mut i = n - 1;
    while i > 0 {
        a.swap(0, i);
        sift(a, 0, i, cmp);
        i -= 1;
    }
}

fn swap_block<E>(a: &mut [E], x: usize, y: usize, span: usize) {
    for k in 0..span {
        a.swap(x + k, y + k);
    }
}

fn rqsort<E>(a: &mut [E], cmp: &mut impl FnMut(&E, &E) -> i32) {
    if a.len() < 2 {
        return;
    }
    let mut stack: Vec<(usize, usize, i32)> = vec![(0, a.len(), 0)];
    while let Some((mut ptr, mut nmemb, mut depth)) = stack.pop() {
        while nmemb > 6 {
            depth += 1;
            if depth > 50 {
                heapsort(&mut a[ptr..ptr + nmemb], cmp);
                nmemb = 0;
                break;
            }
            let m4 = nmemb >> 2;
            let m = med3(a, ptr + m4, ptr + 2 * m4, ptr + 3 * m4, cmp);
            a.swap(ptr, m);
            let mut i = 1usize;
            let mut lt = 1usize;
            let mut pi = ptr + 1;
            let mut plt = ptr + 1;
            let mut gt = nmemb;
            let top = ptr + nmemb;
            let mut pj = top;
            let mut pgt = top;
            loop {
                while pi < pj {
                    let c = cmp(&a[ptr], &a[pi]);
                    if c < 0 {
                        break;
                    }
                    if c == 0 {
                        a.swap(plt, pi);
                        lt += 1;
                        plt += 1;
                    }
                    i += 1;
                    pi += 1;
                }
                loop {
                    pj -= 1;
                    if pi >= pj {
                        break;
                    }
                    let c = cmp(&a[ptr], &a[pj]);
                    if c > 0 {
                        break;
                    }
                    if c == 0 {
                        gt -= 1;
                        pgt -= 1;
                        a.swap(pgt, pj);
                    }
                }
                if pi >= pj {
                    break;
                }
                a.swap(pi, pj);
                i += 1;
                pi += 1;
            }
            let mut span = plt - ptr;
            let span2 = pi - plt;
            lt = i - lt;
            if span > span2 {
                span = span2;
            }
            swap_block(a, ptr, pi - span, span);
            let mut span = top - pgt;
            let span2 = pgt - pi;
            pgt = top - span2;
            gt = nmemb - (gt - i);
            if span > span2 {
                span = span2;
            }
            swap_block(a, pi, top - span, span);
            if lt > nmemb - gt {
                stack.push((ptr, lt, depth));
                ptr = pgt;
                nmemb -= gt;
            } else {
                stack.push((pgt, nmemb - gt, depth));
                nmemb = lt;
            }
        }
        for pi in ptr + 1..ptr + nmemb {
            let mut pj = pi;
            while pj > ptr && cmp(&a[pj - 1], &a[pj]) > 0 {
                a.swap(pj, pj - 1);
                pj -= 1;
            }
        }
    }
}

/// `array.sort(f)`; `None` plays `undefined`, which QuickJS moves to the end without calling `f`.
pub fn sort_by<T: Clone>(v: &mut Vec<Option<T>>, mut f: impl FnMut(&T, &T) -> f64) {
    let undefined = v.iter().filter(|x| x.is_none()).count();
    let mut a: Vec<(T, usize)> = v.iter().enumerate().filter_map(|(i, x)| x.clone().map(|x| (x, i))).collect();
    rqsort(&mut a, &mut |x: &(T, usize), y: &(T, usize)| {
        let r = f(&x.0, &y.0);
        let c = (r > 0.) as i32 - (r < 0.) as i32;
        if c != 0 {
            c
        } else {
            (x.1 > y.1) as i32 - (x.1 < y.1) as i32
        }
    });
    *v = a.into_iter().map(|x| Some(x.0)).chain(std::iter::repeat_n(None, undefined)).collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_consistent_comparator_sorts_stably() {
        let mut v: Vec<Option<(i32, char)>> = [(3, 'a'), (1, 'b'), (3, 'c'), (2, 'd'), (1, 'e'), (5, 'f'), (0, 'g'), (3, 'h'), (2, 'i')].into_iter().map(Some).collect();
        sort_by(&mut v, |a, b| (a.0 - b.0) as f64);
        let order: String = v.iter().map(|x| x.unwrap().1).collect();
        assert_eq!(order, "gbediachf");
    }
}
