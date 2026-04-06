/// A language syntax that can be compiled once.
pub trait Compile {
    type Output;

    /// Compile a given syntax; if already compiled, does nothing. 
    fn compile(&mut self) -> Self::Output;

    /// Returns true if the given syntax is already compiled. 
    fn is_compiled(&self) -> bool;
}