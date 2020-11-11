pub enum Algorithm {
	EnergyImpact,
	SolutionDiversity,
}

pub struct QbsolvParams {
	pub num_repeats: usize, // param.repeats
	pub seed: usize,
	pub verbosity: i32,
	pub algorithm: Algorithm,
	pub timeout: usize,
	pub solver_limit: Option<usize>,
	pub target: Option<f64>,
	pub find_max: bool,
}

unsafe fn unsafe_uninit_vec<T>(n: usize) -> Vec<T> {
	let mut ret = Vec::with_capacity(n);
	ret.set_len(n);
	ret
}

#[allow(dead_code)]
enum Solver<T> {
	Tabu,
	Dw,
	Callback {
		callback: fn(&[&[f64]], usize, &T) -> Vec<bool>,
		data: T,
	},
}

impl QbsolvParams {
	pub fn new() -> Self {
		Self {
			num_repeats: 50,
			seed: 17932241798878,
			verbosity: -1,
			algorithm: Algorithm::EnergyImpact,
			timeout: 2592000,
			solver_limit: None,
			target: None,
			find_max: false,
		}
	}

	/// Solve subqubo with callback.
	pub fn run_with_callback<T>(
		&self,
		q: &[(usize, usize, f64)],
		vals: usize,
		callback: fn(&[&[f64]], usize, &T) -> Vec<bool>,
		data: T,
	) -> Vec<(Vec<bool>, f64, usize)> {
		self.run(q, vals, Solver::Callback { callback, data })
	}

	/// Solve with qbsolv's internal tabu search algorithm.
	pub fn run_internal(
		&self,
		q: &[(usize, usize, f64)],
		vals: usize,
	) -> Vec<(Vec<bool>, f64, usize)> {
		self.run::<()>(q, vals, Solver::Tabu)
	}

	/// Solve with DWave API.
	/// It requires valid installation of qOp in your ${DWAVE_HOME}.
	#[cfg(use_qop)]
	pub fn run_dwave(
		&self,
		q: &[(usize, usize, f64)],
		vals: usize,
	) -> Vec<(Vec<bool>, f64, usize)> {
		self.run::<()>(q, vals, Solver::Dw)
	}

	fn run<T>(
		&self,
		q: &[(usize, usize, f64)],
		vals: usize,
		solver: Solver<T>,
	) -> Vec<(Vec<bool>, f64, usize)> {
		let n_solutions = match self.algorithm {
			Algorithm::EnergyImpact => {
				unsafe {
					ffi::algo_[0] = "o".as_ptr();
					ffi::algo_[1] = std::ptr::null();
				}
				20
			}
			Algorithm::SolutionDiversity => {
				unsafe {
					ffi::algo_[0] = "d".as_ptr();
					ffi::algo_[1] = std::ptr::null();
				}
				70
			}
		};
		unsafe {
			ffi::outFile_ = ffi::stdout;
			ffi::Time_ = self.timeout as f64;
			ffi::Tlist_ = -1;
			ffi::numsolOut_ = 0;
			ffi::Verbose_ = self.verbosity;
			ffi::WriteMatrix_ = false;
			if let Some(target) = self.target {
				ffi::TargetSet_ = true;
				ffi::Target_ = target;
			} else {
				ffi::TargetSet_ = false;
			}
			ffi::findMax_ = self.find_max;
			ffi::srand(self.seed as u32);
		}
		let mut params = unsafe { ffi::default_parameters() };
		params.repeats = self.num_repeats as i32;
		if let Some(solver_limit) = self.solver_limit {
			params.sub_size = solver_limit as i32;
		}

		match &solver {
			Solver::Dw => {
				params.sub_sampler = ffi::dw_sub_sample as unsafe extern "C" fn(_, _, _, _);
				params.sub_size = unsafe { ffi::dw_init() };
			}
			Solver::Callback { .. } => {
				params.sub_sampler =
					Self::subqubo_callback::<T> as unsafe extern "C" fn(_, _, _, _);
				params.sub_sampler_data = &solver as *const Solver<T> as *const _;
			}
			_ => (),
		}

		let mut q_array: Vec<f64> = std::iter::repeat(0.0).take(vals * vals).collect();
		let mut solution_list: Vec<i8> = unsafe { unsafe_uninit_vec(vals * (n_solutions + 1)) };
		let mut energy_list: Vec<f64> = unsafe { unsafe_uninit_vec(n_solutions + 1) };
		let mut solution_counts: Vec<i32> = unsafe { unsafe_uninit_vec(n_solutions + 1) };
		let mut q_index: Vec<i32> = unsafe { unsafe_uninit_vec(n_solutions + 1) };

		let sign = if self.find_max { 1.0 } else { -1.0 };
		for (u, v, bias) in q.iter() {
			if v < u {
				q_array[vals * v + u] = sign * bias;
			} else {
				q_array[vals * u + v] = sign * bias;
			}
		}
		unsafe {
			ffi::solve(
				q_array.as_ptr(),
				vals as i32,
				solution_list.as_mut_ptr(),
				energy_list.as_mut_ptr(),
				solution_counts.as_mut_ptr(),
				q_index.as_mut_ptr(),
				n_solutions as i32,
				&params as *const ffi::paramaters_t,
			);
		}

		let mut ret = Vec::new();

		for i in 0..n_solutions {
			let soln_idx = q_index[i] as usize;
			if solution_counts[soln_idx] == 0 {
				break;
			}
			ret.push((
				solution_list[(soln_idx * vals)..((soln_idx + 1) * vals)]
					.iter()
					.map(|i| i == &0)
					.collect(),
				energy_list[soln_idx] * sign,
				solution_counts[soln_idx] as usize,
			));
		}
		ret
	}

	extern "C" fn subqubo_callback<T>(
		sub_qubo: *const f64,
		vals: i32,
		sub_solution: *mut i8,
		sub_sampler_data: *const std::ffi::c_void,
	) {
		// sub_qubo: vals x vals
		// sub_solution: vals
		let vals = vals as usize;
		let sub_solution = unsafe { std::slice::from_raw_parts_mut(sub_solution, vals) };
		let solver = unsafe { sub_sampler_data.cast::<&Solver<T>>().as_ref().unwrap() };
		if let Solver::Callback { callback, data } = solver {
			let v = (0..vals)
				.map(|i| unsafe {
					std::slice::from_raw_parts(sub_qubo.offset((i * vals) as isize), vals)
				})
				.collect::<Vec<_>>();
			let ret = callback(&v, vals, data);
			assert!(ret.len() == vals);
			for (i, b) in ret.iter().enumerate() {
				sub_solution[i] = if *b { 1 } else { 0 };
			}
		} else {
			panic!()
		}
	}
}

#[test]
fn ffi_test() {
	let params = unsafe { ffi::default_parameters() };
	assert_eq!(params.repeats, 50);
	assert_eq!(params.sub_size, 47);
}

#[allow(non_snake_case)]
mod ffi {
	#[repr(C)]
	pub struct paramaters_t {
		pub repeats: i32,
		pub sub_sampler: unsafe extern "C" fn(*const f64, i32, *mut i8, *const std::ffi::c_void),
		pub sub_size: i32,
		pub sub_sampler_data: *const std::ffi::c_void,
	}

	#[no_mangle]
	#[link(name = "qbsolv")]
	extern "C" {
		pub fn default_parameters() -> paramaters_t;
		pub fn solve(
			qubo: *const f64,
			qubo_size: i32,
			solution_list: *mut i8,
			energy_list: *mut f64,
			solution_counts: *mut i32,
			Qindex: *mut i32,
			QLEN: i32,
			param: *const paramaters_t,
		);
		pub fn dw_sub_sample(
			sub_qubo: *const f64,
			subMatrix: i32,
			sub_solution: *mut i8,
			sub_sampler_data: *const std::ffi::c_void,
		);

		pub fn dw_init() -> i32;
		pub fn srand(seed: u32);
		pub static mut algo_: [*const u8; 2];
		pub static mut Target_: f64;
		pub static mut Time_: f64;
		pub static mut Tlist_: i32;
		pub static mut Verbose_: i32;
		pub static mut numsolOut_: i32;
		pub static mut WriteMatrix_: bool;
		pub static mut TargetSet_: bool;
		pub static mut findMax_: bool;
		pub static mut outFile_: *const u8;
		pub static stdout: *const u8;
	}
}
