use std::path::PathBuf;

use color_eyre::eyre::eyre;

use crate::gl::{self};

pub enum ShaderType {
    Vertex,
    Fragment,
}

pub struct Shader(u32);



impl Shader {
    pub fn from_source(src: &str, shader_type: ShaderType) -> Result<Self, String> {
        
        let id = gl!(gl::CreateShader(match shader_type {
            ShaderType::Vertex => gl::VERTEX_SHADER,
            ShaderType::Fragment => gl::FRAGMENT_SHADER,
        }));
        let c_str = std::ffi::CString::new(src.as_bytes()).unwrap();
        gl!(gl::ShaderSource(id, 1, &c_str.as_ptr(), std::ptr::null()));
        gl!(gl::CompileShader(id));

        // Verificar erros de compilação...
        let mut success: gl::types::GLint = 1;
        gl!(gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success));
        if success == 0 {
            let mut len: gl::types::GLint = 0;
            gl!(gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len));
            let error = create_whitespace_cstring_with_len(len as usize);
            gl!(gl::GetShaderInfoLog(
                id,
                len,
                std::ptr::null_mut(),
                error.as_ptr() as *mut gl::types::GLchar
            ));
            return Err(error.to_string_lossy().into_owned());
        }
        Ok(Shader(id))
    }
    pub fn load_from_file(path: impl Into<PathBuf>) -> color_eyre::Result<Self> {
        let path: PathBuf = path.into();
        
        let shader_string = std::fs::read_to_string(&path)?;
        let shader_type = match path.extension().map_or("", |e| e.to_str().unwrap()) {
            "vs" => Ok(ShaderType::Vertex),
            "fs" => Ok(ShaderType::Fragment),
            ext => Err(eyre!("Unsupported shader extension: {ext:#?}"))
        }?;
        Ok(Shader::from_source(&shader_string, shader_type).map_err(|s| eyre!("Shader compilation error: {s}"))?)
    }
    pub fn id(&self) -> u32 {
        self.0
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        gl!(gl::DeleteShader(self.id()))
    }
}

fn create_whitespace_cstring_with_len(len: usize) -> std::ffi::CString {
    // Cria um CString cheio de espaços para alocar o log de erro
    let buffer: Vec<u8> = vec![b' '; len];
    std::ffi::CString::new(buffer).unwrap()
}

pub struct ShaderProgram(u32);

impl ShaderProgram {
    pub fn from_shaders(shaders: &[Shader]) -> Result<Self, String> {
        let id = gl!(gl::CreateProgram());
        for shader in shaders {
            gl!(gl::AttachShader(id, shader.id()));
        }
        gl!(gl::LinkProgram(id));

        // Verifica se houve erros durante o link
        let mut success: gl::types::GLint = 1;
        gl!(gl::GetProgramiv(id, gl::LINK_STATUS, &mut success));
        if success == 0 {
            let mut len: gl::types::GLint = 0;
            gl!(gl::GetProgramiv(id, gl::INFO_LOG_LENGTH, &mut len));
            let error = create_whitespace_cstring_with_len(len as usize);
            gl!(gl::GetProgramInfoLog(
                id,
                len,
                std::ptr::null_mut(),
                error.as_ptr() as *mut gl::types::GLchar
            ));
            return Err(error.to_string_lossy().into_owned());
        }
        for shader in shaders {
            gl!(gl::DetachShader(id, shader.id()));
        }
        Ok(ShaderProgram(id))
    }
    
}