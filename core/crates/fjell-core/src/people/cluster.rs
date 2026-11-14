//! Klynging av ansiktsembeddings til personer (ren funksjon, ingen I/O).
//!
//! Avhenger av: `crate::model` (`Embedding`, `PersonId`).
//! Grensesnitt: `cosine`, `plan_assignments`, `Target`. Grådig og deterministisk: ansikter behandles i gitt
//! rekkefølge og havner hos nærmeste senter (eksisterende person eller ny klynge) hvis likheten når terskelen,
//! ellers starter de en ny klynge. Senteret er summen av normaliserte embeddings (retning = gjennomsnitt).
//! Appen foreslår grupperinger; brukeren kan slå sammen og splitte (alt angrebart).

use crate::model::{Embedding, PersonId};

/// Hvor et ansikt skal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// En eksisterende person.
    Existing(PersonId),
    /// Ny klynge (nummer fra 0 i denne kjøringen); samme nummer betyr samme nye person.
    New(usize),
}

/// Cosinuslikhet. `None` for ulik dimensjon, tom vektor eller nullnorm.
pub fn cosine(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let (mut dot, mut na, mut nb) = (0f32, 0f32, 0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= 0.0 || nb <= 0.0 || !dot.is_finite() {
        return None;
    }
    Some(dot / (na.sqrt() * nb.sqrt()))
}

fn normalized(e: &Embedding) -> Option<Vec<f32>> {
    let n = e.0.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n <= 0.0 || !n.is_finite() || e.0.is_empty() {
        return None;
    }
    Some(e.0.iter().map(|x| x / n).collect())
}

struct Centroid {
    target: Target,
    sum: Vec<f32>,
}

/// Fordeler `new` (id, embedding) på `existing` (person, senter) eller nye klynger. Ansikter med ugyldig
/// embedding (nullnorm eller tom) får ingen tildeling og utelates. Ulik dimensjon mot et senter regnes som
/// «ikke lik» (starter ny klynge).
pub fn plan_assignments(
    existing: &[(PersonId, Embedding)],
    new: &[(u64, Embedding)],
    threshold: f32,
) -> Vec<(u64, Target)> {
    let mut cs: Vec<Centroid> = existing
        .iter()
        .filter_map(|(p, e)| {
            normalized(e).map(|sum| Centroid {
                target: Target::Existing(*p),
                sum,
            })
        })
        .collect();
    let mut next_new = 0usize;
    let mut out = Vec::with_capacity(new.len());
    for (id, e) in new {
        let Some(v) = normalized(e) else { continue };
        let mut best: Option<(usize, f32)> = None;
        for (i, c) in cs.iter().enumerate() {
            if let Some(s) = cosine(&c.sum, &v) {
                // Strengt større: ved likhet vinner den første (deterministisk).
                if s >= threshold && best.is_none_or(|(_, bs)| s > bs) {
                    best = Some((i, s));
                }
            }
        }
        match best {
            Some((i, _)) => {
                for (a, b) in cs[i].sum.iter_mut().zip(&v) {
                    *a += b;
                }
                out.push((*id, cs[i].target));
            }
            None => {
                let t = Target::New(next_new);
                next_new += 1;
                cs.push(Centroid { target: t, sum: v });
                out.push((*id, t));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(v: &[f32]) -> Embedding {
        Embedding(v.to_vec())
    }

    #[test]
    fn cosine_basics() {
        assert_eq!(cosine(&[1.0, 0.0], &[2.0, 0.0]), Some(1.0));
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 1.0]), Some(0.0));
        assert_eq!(cosine(&[1.0], &[1.0, 0.0]), None);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), None);
        assert_eq!(cosine(&[], &[]), None);
    }

    #[test]
    fn groups_similar_and_separates_different() {
        let new = [
            (1, e(&[1.0, 0.0])),
            (2, e(&[0.0, 1.0])),
            (3, e(&[0.99, 0.05])),
            (4, e(&[0.05, 0.99])),
        ];
        let out = plan_assignments(&[], &new, 0.8);
        let t: Vec<_> = out.iter().map(|x| x.1).collect();
        assert_eq!(
            t,
            vec![
                Target::New(0),
                Target::New(1),
                Target::New(0),
                Target::New(1)
            ]
        );
    }

    #[test]
    fn prefers_existing_person_and_is_deterministic() {
        let existing = [(PersonId(7), e(&[1.0, 0.0])), (PersonId(8), e(&[0.0, 1.0]))];
        let new = [
            (1, e(&[0.9, 0.1])),
            (2, e(&[0.1, 0.9])),
            (3, e(&[-1.0, -1.0])),
        ];
        let a = plan_assignments(&existing, &new, 0.8);
        assert_eq!(a[0].1, Target::Existing(PersonId(7)));
        assert_eq!(a[1].1, Target::Existing(PersonId(8)));
        assert_eq!(a[2].1, Target::New(0));
        assert_eq!(a, plan_assignments(&existing, &new, 0.8));
    }

    #[test]
    fn invalid_embeddings_are_skipped_not_assigned() {
        let new = [(1, e(&[0.0, 0.0])), (2, e(&[])), (3, e(&[1.0, 0.0]))];
        let out = plan_assignments(&[], &new, 0.5);
        assert_eq!(out, vec![(3, Target::New(0))]);
    }

    #[test]
    fn different_dimension_starts_new_cluster() {
        let new = [(1, e(&[1.0, 0.0])), (2, e(&[1.0, 0.0, 0.0]))];
        let out = plan_assignments(&[], &new, 0.5);
        assert_eq!(out[1].1, Target::New(1));
    }
}
