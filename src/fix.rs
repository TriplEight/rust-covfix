use regex::Regex;
use std::fs;

use crate::common::{BranchCoverage, FileCoverage, LineCoverage, PackageCoverage, SourceCode};

struct State {
    is_test: bool,
}

impl State {
    fn new() -> Self {
        State { is_test: false }
    }
}

pub struct Fixer {
    ne_reg: Vec<Regex>,
    p_reg: Vec<Regex>,
    ts_reg: Vec<Regex>,
}

impl Fixer {
    pub fn new() -> Self {
        Self {
            ne_reg: vec![
                Regex::new(
                    r"^(?:\s*\}(?:\s*\))*(?:\s*;)?|\s*(?:\}\s*)?else(?:\s*\{)?)?\s*(?://.*)?$",
                )
                .unwrap(),
                Regex::new(r"^\s*pub\s*struct\s*.*?\{\s*(?://.*)?$").unwrap(),
                Regex::new(r"^\s*pub\s*enum\s*.*?\{\s*(?://.*)?$").unwrap(),
            ],
            p_reg: vec![
                Regex::new(r"^\s*for\s*.*\{\s*(?://.*)?$").unwrap(),
                Regex::new(r"^\s*while\s*.*\{\s*(?://.*)?$").unwrap(),
            ],
            ts_reg: vec![Regex::new(r"^\s*mod\s*test\s*\{\s*(?://.*)?$").unwrap()],
        }
    }

    /// fix coverage information
    pub fn fix(&self, data: &mut PackageCoverage) {
        for mut file_cov in &mut data.file_coverages {
            let path = file_cov.path();
            if !path.is_file() {
                panic!("Source file not found: {:?}", path);
            }

            let content = fs::read_to_string(path).unwrap();
            let source = SourceCode::new(content);

            self.process_file(&source, &mut file_cov);
        }
    }

    // thread unsafe method
    fn process_file(&self, source: &SourceCode, cov: &mut FileCoverage) {
        cov.line_coverages.sort_unstable_by_key(|v| v.line_number);
        cov.branch_coverages.sort_unstable_by_key(|v| v.line_number);

        let mut state = State::new();

        unsafe {
            let mut lp = cov.line_coverages.as_mut_ptr();
            let lp_end = lp.add(cov.line_coverages.len());

            let mut bp = cov.branch_coverages.as_mut_ptr();
            let bp_end = bp.add(cov.branch_coverages.len());

            // skip branch coverages which does not contains line information
            while bp < bp_end && (*bp).line_number.is_none() {
                bp = bp.add(1);
            }

            for (line, line_str) in source.lines().enumerate() {
                // line coverage at current line
                let line_cov = if lp < lp_end && (*lp).line_number == line {
                    let val = Some(&mut *lp);
                    lp = lp.add(1);
                    val
                } else {
                    None
                };

                // branch coverages at current line
                let branch_covs = if bp < bp_end && (*bp).line_number.unwrap() == line {
                    let start = bp;
                    bp = bp.add(1);
                    let mut count = 1;
                    while bp < bp_end && (*bp).line_number.unwrap() == line {
                        bp = bp.add(1);
                        count += 1;
                    }
                    Some(std::slice::from_raw_parts_mut(start, count))
                } else {
                    None
                };

                // fix coverage
                self.process_line(line_str, line_cov, branch_covs, &mut state);
            }
        }
    }

    fn process_line(
        &self,
        line: &str,
        line_cov: Option<&mut LineCoverage>,
        branch_covs: Option<&mut [BranchCoverage]>,
        state: &mut State,
    ) {
        if state.is_test {
            return;
        }

        if self.ts_reg.iter().any(|r| r.is_match(line)) {
            state.is_test = true;
            return;
        }

        if line_cov.is_none() && branch_covs.is_none() {
            return;
        }

        if self.ne_reg.iter().any(|r| r.is_match(line)) {
            if let Some(&mut ref mut line_cov) = line_cov {
                line_cov.count = None
            };
            if let Some(&mut ref mut branch_covs) = branch_covs {
                branch_covs.iter_mut().for_each(|v| v.taken = false);
            }
        }

        if let Some(&mut ref mut branch_covs) = branch_covs {
            let should_be_fixed = match line_cov {
                Some(&mut LineCoverage { count: None, .. }) => false,
                _ => true,
            };
            if should_be_fixed && self.p_reg.iter().any(|r| r.is_match(line)) {
                branch_covs.iter_mut().for_each(|v| v.taken = true);
            }
        }
    }
}

impl Default for Fixer {
    fn default() -> Self {
        Self::new()
    }
}
