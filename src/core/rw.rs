macro_rules! tcp_stream_start {
    ($reader:ident, $buf:ident) => {
        let _ = $reader.readable().await;
        loop {
            match $reader.try_read_buf(&mut $buf) {
                Ok(n) => {
                    trace!("tcp_stream_start {}", n);
                    if n == 0 {
                        break;
                    }
                    continue;
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("tcp_stream_start WouldBlock");
                        break;
                    } else {
                        error!("tcp_stream_start error: {:?}", e);
                        break;
                    }
                }
            }
        }
    };
}

macro_rules! tcp_stream_read {
    ($reader:ident, $buf:ident, $func:ident, $wr:ident, $state:ident, $str:literal) => {
        loop {
            match $reader.try_read_buf(&mut $buf) {
                Ok(n) => {
                    trace!("{} tcp_stream_read {}", $str, n);
                    if n > 0 {
                        $func.data(&mut $buf).await;
                        if let Err(e) = $wr.write_all(&$buf).await {
                            error!("{} write error: {:?}", $str, e);
                            $state.2 = true;
                        }
                        $buf.clear();
                        continue;
                    } else {
                        $state.0 = true;
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("{} tcp_stream_read WouldBlock", $str);
                        $state.1 = true;
                        break;
                    } else {
                        error!("{} tcp_stream_read error: {:?}", $str, e);
                        $state.0 = true;
                        break;
                    }
                }
            }
        }
        if $state.0 || $state.1 {
            $func.enddata(&mut $buf).await;
        }
        if $buf.len() > 0 {
            if let Err(e) = $wr.write_all(&$buf).await {
                $state.2 = true;
                error!("{} write error: {:?}", $str, e);
            }
            $buf.clear();
        }
    };
}

macro_rules! async_ext_start {
    ($reader:ident, $buf:ident) => {
        match $reader.read_buf(&mut $buf).await {
            Ok(n) => {
                trace!("async_ext_start {}", n);
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    debug!("async_ext_start WouldBlock");
                } else {
                    error!("async_ext_start error: {:?}", e);
                }
            }
        }
    };
}

macro_rules! async_ext_read {
    ($read:ident, $buf:ident, $func:ident, $wr:ident, $state:ident, $str:literal) => {
        match $read {
            Ok(n) => {
                trace!("{} async_ext_read {}", $str, n);
                if n > 0 {
                    $func.data(&mut $buf).await;
                    if let Err(e) = $wr.write_all(&$buf).await {
                        error!("{} write error: {:?}", $str, e);
                        $state.2 = true;
                    }
                    $buf.clear();
                } else {
                    $state.0 = true;
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    debug!("{} async_ext_read WouldBlock", $str);
                    $state.1 = true;
                } else {
                    error!("{} async_ext_read error:{:?}", $str, e);
                    $state.0 = true;
                }
            }
        }

        if $state.0 || $state.1 {
            $func.enddata(&mut $buf).await;
        }
        if $buf.len() > 0 {
            if let Err(e) = $wr.write_all(&$buf).await {
                error!("{} write error: {:?}", $str, e);
                $state.2 = true;
            }
            $buf.clear();
        }
    };
}

macro_rules! async_ext_rw {
    ($server:ident, $r_buf:ident, $w_buf:ident, $func:ident, $state:ident) => {
        loop {
            match $server.read_buf(&mut $r_buf).await {
                Ok(n) => {
                    trace!("async_ext_rw {}", n);
                    if n > 0 {
                        if $r_buf.len() > 0 {
                            $func.service(&mut $r_buf, &mut $w_buf).await;
                            $r_buf.clear();
                        }
                        if $w_buf.len() > 0 {
                            if let Err(e) = $server.write_all(&$w_buf).await {
                                error!("async_ext_rw write error: {:?}", e);
                                $state.2 = true;
                            }
                            $w_buf.clear();
                        }
                        continue;
                    } else {
                        $state.0 = true;
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("async_ext_rw WouldBlock");
                        $state.1 = true;
                        break;
                    } else {
                        error!("async_ext_rw read error:{:?}", e);
                        $state.0 = true;
                        break;
                    }
                }
            }
        }
        if $r_buf.len() > 0 {
            $func.service(&mut $r_buf, &mut $w_buf).await;
            $r_buf.clear();
        }
        if $w_buf.len() > 0 {
            if let Err(e) = $server.write_all(&$w_buf).await {
                error!("async_ext_rw write error: {:?}", e);
                $state.2 = true;
            }
            $w_buf.clear();
        }
    };
}
