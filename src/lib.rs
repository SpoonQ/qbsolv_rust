/// identify methods implemented in the qbsolv library
pub enum Solver {
	/// built-in tabu sub-problem solver
	Tabu,
	/// built-in dw interface sub-problem solver
	Dw,
}

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
	pub solver_limit: Option<()>,
	pub solver: Solver,
	pub target: Option<f64>,
	pub find_max: bool,
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
			solver: Solver::Tabu,
			target: None,
			find_max: false,
		}
	}

	pub fn run(&self, q: &[(usize, usize, f64)], vals: usize) -> Vec<(Vec<bool>, f64, usize)> {
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
		if let Solver::Dw = self.solver {
			unsafe {
				params.sub_sampler = ffi::dw_sub_sample as unsafe extern "C" fn(_, _, _, _);
				params.sub_size = ffi::dw_init();
			}
		}

		let mut q_array: Vec<f64> = std::iter::repeat(0.0).take(vals * vals).collect();

		// TODO: initialize of followings are not needed
		let mut solution_list: Vec<i8> = std::iter::repeat(0)
			.take(vals * (n_solutions + 1))
			.collect();
		let mut energy_list: Vec<f64> = std::iter::repeat(0.0).take(n_solutions + 1).collect();
		let mut solution_counts: Vec<i32> = std::iter::repeat(0).take(n_solutions + 1).collect();
		let mut q_index: Vec<i32> = std::iter::repeat(0).take(n_solutions + 1).collect();

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
		pub sub_sampler: unsafe extern "C" fn(*mut f64, i32, *mut i8, *mut u8),
		pub sub_size: i32,
		pub sub_sampler_data: *mut u8,
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
			sub_qubo: *mut f64,
			subMatrix: i32,
			sub_solution: *mut i8,
			sub_sampler_data: *mut u8,
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
