use std::ffi::CString;
use std::io::{Read, Result, Write};
pub use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, ToSocketAddrs};
use std::str;

#[repr(C)]
pub struct IovecRead {
    pub buf: *mut libc::c_uchar,
    pub size: usize,
}

#[repr(C)]
pub struct IovecWrite {
    pub buf: *const libc::c_uchar,
    pub size: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WasiAddress {
    pub buf: *const libc::c_uchar,
    pub size: usize,
}

unsafe impl Send for WasiAddress {}

#[derive(Debug)]
#[repr(C)]
pub struct WasiSockaddr {
    pub family: AddressFamily,
    pub sa_data_len: u32,
    pub sa_data: *mut libc::c_uchar,
}

impl WasiSockaddr {
    pub fn new(family: AddressFamily, sa_data: &mut Vec<u8>) -> WasiSockaddr {
        WasiSockaddr {
            family,
            sa_data_len: 14,
            sa_data: sa_data.as_mut_ptr(),
        }
    }
}

#[link(wasm_import_module = "wasi_snapshot_preview1")]
extern "C" {
    pub fn sock_open(addr_family: u8, sock_type: u8, fd: *mut u32) -> u32;
    pub fn sock_close(fd: u32);
    pub fn sock_bind(fd: u32, addr: *mut WasiAddress, port: u32) -> u32;
    pub fn sock_listen(fd: u32, backlog: u32) -> u32;
    pub fn sock_accept(fd: u32, fd: *mut u32) -> u32;
    pub fn sock_connect(fd: u32, addr: *mut WasiAddress, port: u32) -> u32;
    pub fn sock_recv(
        fd: u32,
        buf: *mut IovecRead,
        buf_len: usize,
        flags: u16,
        recv_len: *mut usize,
        oflags: *mut usize,
    ) -> u32;
    pub fn sock_recv_from(
        fd: u32,
        buf: *mut u8,
        buf_len: u32,
        addr: *mut u8,
        addr_len: *mut u32,
        flags: u16,
    ) -> u32;
    pub fn sock_send(
        fd: u32,
        buf: *const IovecWrite,
        buf_len: u32,
        flags: u16,
        send_len: *mut u32,
    ) -> u32;
    pub fn sock_send_to(
        fd: u32,
        buf: *const u8,
        buf_len: u32,
        addr: *const u8,
        addr_len: u32,
        flags: u16,
    ) -> u32;
    pub fn sock_shutdown(fd: u32, flags: u8) -> u32;
    pub fn sock_getaddrinfo(
        node: *const u8,
        node_len: u32,
        server: *const u8,
        server_len: u32,
        hint: *const WasiAddrinfo,
        res: *mut u32,
        max_len: u32,
        res_len: *mut u32,
    ) -> u32;
    pub fn sock_getpeeraddr(
        fd: u32,
        addr: *mut WasiAddress,
        addr_type: *mut u32,
        port: *mut u32,
    ) -> u32;
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum AddressFamily {
    Inet4,
    Inet6,
}

impl From<SocketAddr> for AddressFamily {
    fn from(addr: SocketAddr) -> AddressFamily {
        match addr {
            SocketAddr::V4(_) => AddressFamily::Inet4,
            SocketAddr::V6(_) => AddressFamily::Inet6,
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum SocketType {
    Datagram,
    Stream,
}

#[derive(Copy, Clone, Debug)]
#[repr(u16)]
pub enum AiFlags {
    AiPassive,
    AiCanonname,
    AiNumericHost,
    AiNumericServ,
    AiV4Mapped,
    AiAll,
    AiAddrConfig,
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum AiProtocol {
    IPProtoTCP,
    IPProtoUDP,
}
#[derive(Debug, Clone)]
#[repr(C)]
pub struct WasiAddrinfo {
    pub ai_flags: AiFlags,
    pub ai_family: AddressFamily,
    pub ai_socktype: SocketType,
    pub ai_protocol: AiProtocol,
    pub ai_addrlen: u32,
    pub ai_addr: *mut WasiSockaddr,
    pub ai_canonname: *mut u8,
    pub ai_canonnamelen: u32,
    pub ai_next: *mut WasiAddrinfo,
}

impl WasiAddrinfo {
    pub fn default() -> WasiAddrinfo {
        WasiAddrinfo {
            ai_flags: AiFlags::AiPassive,
            ai_family: AddressFamily::Inet4,
            ai_socktype: SocketType::Datagram,
            ai_protocol: AiProtocol::IPProtoTCP,
            ai_addr: std::ptr::null_mut(),
            ai_addrlen: 0,
            ai_canonname: std::ptr::null_mut(),
            ai_canonnamelen: 0,
            ai_next: std::ptr::null_mut(),
        }
    }

    /// Get Address Information
    ///
    /// As calling FFI, use buffer as parameter in order to avoid memory leak.
    pub fn get_addrinfo(
        node: &str,
        service: &str,
        hints: &WasiAddrinfo,
        max_reslen: usize,
        sockaddr: &mut Vec<WasiSockaddr>,
        sockbuff: &mut Vec<Vec<u8>>,
        ai_canonname: &mut Vec<String>,
    ) -> Result<Vec<WasiAddrinfo>> {
        let mut res_len: u32 = 0;
        let mut wasiaddrinfo_array: Vec<WasiAddrinfo> = Vec::<WasiAddrinfo>::new();
        for i in 0..max_reslen {
            sockbuff.push(vec![0u8; 14]);
            sockaddr.push(WasiSockaddr::new(AddressFamily::Inet4, &mut sockbuff[i]));
            ai_canonname.push(String::with_capacity(30));
            wasiaddrinfo_array.push(WasiAddrinfo {
                ai_flags: AiFlags::AiPassive,
                ai_family: AddressFamily::Inet6,
                ai_socktype: SocketType::Datagram,
                ai_protocol: AiProtocol::IPProtoTCP,
                ai_addr: &mut sockaddr[i],
                ai_addrlen: 0,
                ai_canonname: ai_canonname[i].as_mut_ptr(),
                ai_canonnamelen: 30,
                ai_next: std::ptr::null_mut(),
            });
        }
        let mut res = wasiaddrinfo_array.as_mut_ptr() as u32;
        unsafe {
            let return_code = sock_getaddrinfo(
                node.as_ptr(),
                node.len() as u32,
                service.as_ptr(),
                service.len() as u32,
                hints as *const WasiAddrinfo,
                &mut res,
                max_reslen as u32,
                &mut res_len,
            );
            match return_code {
                0 => Ok(wasiaddrinfo_array[..res_len as usize].to_vec()),
                _ => Err(std::io::Error::last_os_error()),
            }
        }
    }
}

trait AsRawFd {
    fn as_raw_fd(&self) -> u32;
}

#[derive(Copy, Clone, Debug)]
struct SocketHandle(u32);

impl AsRawFd for SocketHandle {
    fn as_raw_fd(&self) -> u32 {
        self.0
    }
}

#[non_exhaustive]
#[derive(Copy, Clone, Debug)]
pub struct TcpStream {
    fd: SocketHandle,
}

#[non_exhaustive]
#[derive(Copy, Clone, Debug)]
pub struct TcpListener {
    fd: SocketHandle,
    pub address: WasiAddress,
    pub port: u16,
}

#[non_exhaustive]
pub struct UdpSocket {
    fd: SocketHandle,
}

macro_rules !impl_as_raw_fd {
  ($($t : ident) *) => {$(
        impl AsRawFd for $t {
            fn as_raw_fd(&self) -> u32 {
                self.fd.as_raw_fd()
            }
        }
    )*
  };
}

impl_as_raw_fd! {TcpStream TcpListener UdpSocket}

impl TcpStream {
    /// Create TCP socket and connect to the given address.
    ///
    /// If multiple address is given, the first successful socket is
    /// returned.
    pub fn connect<A: ToSocketAddrs>(addrs: A) -> Result<TcpStream> {
        match addrs.to_socket_addrs()?.find_map(|addrs| unsafe {
            let mut fd: u32 = 0;
            sock_open(
                AddressFamily::from(addrs) as u8,
                SocketType::Stream as u8,
                &mut fd,
            );
            let addr_s = addrs.to_string();
            let addrp: Vec<&str> = addr_s.split(':').collect();
            let vaddr: Vec<u8> = addrp[0]
                .split('.')
                .map(|x| x.parse::<u8>().unwrap())
                .collect();
            let port: u16 = addrp[1].parse::<u16>().unwrap();
            let mut addr = WasiAddress {
                buf: vaddr.as_ptr(),
                size: 4,
            };

            sock_connect(fd, &mut addr, port as u32);

            Some(SocketHandle(fd))
        }) {
            Some(fd) => Ok(TcpStream { fd }),
            _ => Err(std::io::Error::last_os_error()),
        }
    }

    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        unsafe {
            sock_shutdown(self.as_raw_fd(), how as u8);
        }
        Ok(())
    }

    pub fn peer_addr(&self) -> Result<SocketAddr> {
        let buf: Vec<u8> = Vec::with_capacity(4);
        let mut addr = WasiAddress {
            buf: buf.as_ptr(),
            size: 16,
        };
        let mut addr_type = 0;
        let mut port = 0;
        unsafe {
            sock_getpeeraddr(self.as_raw_fd(), &mut addr, &mut addr_type, &mut port);
            let addr = std::slice::from_raw_parts(addr.buf, 4);
            if addr_type != 4 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "unsupported address type",
                ));
            }
            let ret = SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3])),
                port as u16,
            );
            Ok(ret)
        }
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let flags = 0;
        let mut recv_len: usize = 0;
        let mut oflags: usize = 0;
        let mut vec = IovecRead {
            buf: buf.as_mut_ptr(),
            size: buf.len(),
        };

        unsafe {
            sock_recv(
                self.as_raw_fd(),
                &mut vec,
                1,
                flags,
                &mut recv_len,
                &mut oflags,
            );
        };
        Ok(recv_len)
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let sent = unsafe {
            let mut send_len: u32 = 0;
            let vec = IovecWrite {
                buf: buf.as_ptr(),
                size: buf.len(),
            };
            sock_send(self.as_raw_fd(), &vec, 1, 0, &mut send_len);
            send_len
        };
        Ok(sent as usize)
    }
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl TcpListener {
    /// Create TCP socket and bind to the given address.
    ///
    /// If multiple address is given, the first successful socket is
    /// returned.
    pub fn bind<A: ToSocketAddrs>(addrs: A) -> Result<TcpListener> {
        match addrs.to_socket_addrs()?.find_map(|addrs| unsafe {
            let mut fd: u32 = 0;
            sock_open(
                AddressFamily::from(addrs) as u8,
                SocketType::Stream as u8,
                &mut fd,
            );
            let addr_s = addrs.to_string();
            let addrp: Vec<&str> = addr_s.split(':').collect();
            let vaddr: Vec<u8> = addrp[0]
                .split('.')
                .map(|x| x.parse::<u8>().unwrap())
                .collect();
            let port: u16 = addrp[1].parse::<u16>().unwrap();
            let mut addr = WasiAddress {
                buf: vaddr.as_ptr(),
                size: 4,
            };

            sock_bind(fd, &mut addr, port as u32);
            sock_listen(fd, 128);
            Some((SocketHandle(fd), addr, port))
        }) {
            Some((fd, addr, port)) => Ok(TcpListener {
                fd,
                address: addr,
                port,
            }),
            _ => Err(std::io::Error::last_os_error()),
        }
    }
    pub fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        unsafe {
            let mut fd: u32 = 0;
            sock_accept(self.as_raw_fd(), &mut fd);
            let fd = SocketHandle(fd);
            let tcpstream = TcpStream { fd };
            let peer_addr = tcpstream.peer_addr()?;
            Ok((tcpstream, peer_addr))
        }
    }

    pub fn incoming(&self) -> Incoming<'_> {
        Incoming { listener: self }
    }
}

impl<'a> Iterator for Incoming<'a> {
    type Item = Result<TcpStream>;

    fn next(&mut self) -> Option<Result<TcpStream>> {
        Some(self.listener.accept().map(|s| s.0))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

pub struct Incoming<'a> {
    listener: &'a TcpListener,
}

impl UdpSocket {
    /// Create UDP socket and bind to the given address.
    ///
    /// If multiple address is given, the first successful socket is
    /// returned.
    pub fn bind<A: ToSocketAddrs>(_addrs: A) -> Result<UdpSocket> {
        todo!();
    }
    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        let mut addr_len: u32 = 0;
        let mut addr_buf = [0; 32];
        let size = unsafe {
            sock_recv_from(
                self.as_raw_fd(),
                buf.as_ptr() as *mut u8,
                buf.len() as u32,
                addr_buf.as_ptr() as *mut u8,
                &mut addr_len,
                0,
            )
        } as usize;
        let addr_buf = &mut addr_buf[..size];
        Ok((
            size,
            CString::new(addr_buf)
                .expect("CString::new")
                .into_string()
                .expect("CString::into_string")
                .parse::<SocketAddr>()
                .expect("String::parse::<SocketAddr>"),
        ))
    }
    pub fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addr: A) -> Result<usize> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "No address."));
        let addr_s = CString::new(addr?.to_string()).expect("CString::new");
        let sent = unsafe {
            sock_send_to(
                self.as_raw_fd(),
                buf.as_ptr() as *const u8,
                buf.len() as u32,
                addr_s.as_ptr() as *const u8,
                addr_s.as_bytes().len() as u32,
                0,
            )
        } as usize;
        Ok(sent)
    }
}
