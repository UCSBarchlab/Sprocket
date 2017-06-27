use rtl8139;
use timer;
use ide;
use alloc::borrow::ToOwned;

pub trait Service {
    fn name() -> &'static str; // service name
    fn start(); // called after OS has set up paging, allocator, etc.
}

pub struct UserService;

impl Service for UserService {
    fn name() -> &'static str {
        "COFFLOS HTTP Demo"
    }

    fn start() {
        info!("Reading root fs");

        use file;
        use file::{UnixFileSystem, FileHandle};
        let fs = file::SimpleFs::new(ide::Ide::init());
        let mut file = fs.open(b"/", b"README");

        let mut buf = vec![0; file.size()];
        file.read(&mut buf);
        if let Ok(s) = ::core::str::from_utf8(&buf) {
            info!("{}", s);
        }


        use alloc::string::String;

        let mut file = fs.open(b"/", b"index.html");
        let mut buf = vec![0; file.size()];
        file.read(&mut buf);
        let html =
            String::from_utf8(buf).unwrap().replace("${{VERSION}}", env!("CARGO_PKG_VERSION"));

        let header: String = "HTTP/1.1 200 OK\r\n\r\n".to_owned();
        let http = header + html.as_str();

        loop {
            use smoltcp::iface::{EthernetInterface, SliceArpCache, ArpCache};
            use smoltcp::wire::{EthernetAddress, IpAddress};
            use smoltcp::socket::{AsSocket, SocketSet};
            use smoltcp::socket::{TcpSocket, TcpSocketBuffer};
            use smoltcp::Error;
            use alloc::boxed::Box;
            use core::str;

            let arp_cache = SliceArpCache::new(vec![Default::default(); 8]);
            let hw_addr = unsafe { EthernetAddress(rtl8139::NIC.as_mut().unwrap().mac_address()) };

            let protocol_addr = IpAddress::v4(10, 0, 0, 4);
            let nic = unsafe { rtl8139::NIC.as_mut().unwrap() };
            let mut iface = EthernetInterface::new(nic,
                                                   Box::new(arp_cache) as Box<ArpCache>,
                                                   hw_addr,
                                                   [protocol_addr]);

            let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; 4096]);
            let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; 4096]);
            let tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

            let mut sockets = SocketSet::new(vec![]);
            let tcp_handle = sockets.add(tcp_socket);

            loop {
                {
                    let socket: &mut TcpSocket = sockets.get_mut(tcp_handle).as_socket();
                    if !socket.is_open() {
                        socket.listen(80).unwrap();
                    }

                    if socket.can_recv() {
                        let _ = socket.recv(200);
                        if socket.can_send() {
                            let seconds = unsafe { timer::TICKS } / 100;
                            const SECONDS_PER_MINUTE: u32 = 60;
                            const MINUTES_PER_HOUR: u32 = 60;
                            const HOURS_PER_DAY: u32 = 24;
                            const SECONDS_PER_DAY: u32 = (SECONDS_PER_MINUTE * MINUTES_PER_HOUR *
                                                          HOURS_PER_DAY);
                            const SECONDS_PER_HOUR: u32 = (SECONDS_PER_MINUTE * MINUTES_PER_HOUR);

                            let days = seconds / SECONDS_PER_DAY;
                            let hours = (seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
                            let minutes = (seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
                            let seconds = seconds % SECONDS_PER_MINUTE;

                            let time =
                                format!("{} days, {}:{:02}:{:02}", days, hours, minutes, seconds);

                            socket.send_slice(http.replace("${{TIME}}", &time).as_str().as_bytes())
                                .unwrap();
                            info!("socket closing");
                            socket.close();
                        }
                    }
                }

                match iface.poll(&mut sockets, 10) {
                    Ok(()) | Err(Error::Exhausted) => (),
                    Err(e) => warn!("poll error: {}", e),
                }
            }

        }

    }
}
