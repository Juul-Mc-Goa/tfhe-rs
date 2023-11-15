use crate::ciphertext::{FheAsciiChar, FheStrLength, FheString, Padding};
use crate::server_key::StringServerKey;
use tfhe::integer::RadixCiphertext;

impl StringServerKey {
    /// Check if s1 and s2 encrypt the same string, for s1 and s2 FheString.
    /// Return an encrypted value of 1 for true.
    pub fn eq(&self, s1: &FheString, s2: &FheString) -> RadixCiphertext {
        match (&s1.length, &s2.length) {
            (&FheStrLength::Clear(l1), &FheStrLength::Clear(l2)) if l1 != l2 => {
                return self.create_zero()
            }
            _ => (),
        }

        match (s1.padding, s2.padding) {
            (Padding::None | Padding::Final, Padding::None | Padding::Final) => {
                self.eq_no_init_padding(s1, s2)
            }
            (Padding::None | Padding::Final, _) => {
                self.eq_no_init_padding(s1, &self.remove_initial_padding(s2))
            }
            (_, Padding::None | Padding::Final) => {
                self.eq_no_init_padding(&self.remove_initial_padding(s1), s2)
            }
            _ => self.eq_no_init_padding(
                &self.remove_initial_padding(s1),
                &self.remove_initial_padding(s2),
            ),
        }
    }

    /// Check if s1 encrypts a string which has the string encrypted by `prefix` as a prefix. Return
    /// an encrypted value of 1 for true and an encrypted value of 0 for false.
    pub fn starts_with_encrypted(&self, s: &FheString, prefix: &FheString) -> RadixCiphertext {
        // If the prefix is longer than the encrypted string, return false
        match (&s.length, &prefix.length) {
            (&FheStrLength::Clear(l), &FheStrLength::Clear(l_prefix)) if l_prefix > l => {
                return self.create_zero()
            }
            (_, &FheStrLength::Clear(l_prefix)) if l_prefix > s.content.len() => {
                return self.create_zero()
            }
            _ => (),
        }

        match (s.padding, prefix.padding) {
            (Padding::None | Padding::Final, Padding::None | Padding::Final) => {
                self.starts_with_encrypted_no_init_padding(s, prefix)
            }
            (Padding::None | Padding::Final, _) => {
                self.starts_with_encrypted_no_init_padding(s, &self.remove_initial_padding(prefix))
            }
            (_, Padding::None | Padding::Final) => {
                self.starts_with_encrypted_no_init_padding(&self.remove_initial_padding(s), prefix)
            }
            _ => self.starts_with_encrypted_no_init_padding(
                &self.remove_initial_padding(s),
                &self.remove_initial_padding(prefix),
            ),
        }
    }

    /// Check if s1 encrypt the string s2, for s1 an FheString and s2 a clear &str.
    /// Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    pub fn eq_clear(&self, s1: &FheString, s2: &str) -> RadixCiphertext {
        match s1.length {
            FheStrLength::Clear(l1) if l1 != s2.len() => return self.create_zero(),
            _ => (),
        }
        return match s1.padding {
            Padding::None | Padding::Final => self.eq_clear_no_init_padding(s1, s2),
            _ => self.eq_clear_no_init_padding(&self.remove_initial_padding(s1), s2),
        };
    }

    /// Check if s1 encrypts a string which has the clear string `prefix` as a prefix. Return an
    /// encrypted value of 1 for true and an encrypted value of 0 for false.
    pub fn starts_with_clear(&self, s: &FheString, prefix: &str) -> RadixCiphertext {
        match s.length {
            FheStrLength::Clear(length) if prefix.len() > length => return self.create_zero(),
            _ if prefix.len() > s.content.len() => return self.create_zero(),
            _ => (),
        }
        return match s.padding {
            Padding::None | Padding::Final => self.starts_with_clear_no_init_padding(s, prefix),
            _ => self.starts_with_clear_no_init_padding(&self.remove_initial_padding(s), prefix),
        };
    }

    /// Check if s1 and s2 encrypt the same string, for s1 and s2 FheString with no initial padding
    /// zeros. Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    pub fn eq_no_init_padding(&self, s1: &FheString, s2: &FheString) -> RadixCiphertext {
        // First the content are compared
        let mut result = self.create_true();
        for n in 0..std::cmp::min(s1.content.len(), s2.content.len()) {
            self.integer_key.bitand_assign_parallelized(
                &mut result,
                &self.compare_char(&s1.content[n], &s2.content[n], std::cmp::Ordering::Equal),
            )
        }

        // If content sizes mismatch, check if the extra characters are padding zeros
        if s1.content.len() > s2.content.len() {
            return self.integer_key.bitand_parallelized(
                &result,
                &self
                    .integer_key
                    .scalar_eq_parallelized(&s1.content[s2.content.len()].0, 0),
            );
        }
        if s2.content.len() > s1.content.len() {
            return self.integer_key.bitand_parallelized(
                &result,
                &self
                    .integer_key
                    .scalar_eq_parallelized(&s2.content[s1.content.len()].0, 0),
            );
        }
        result
    }

    /// Check if s1 encrypts a string which has the string encrypted by prefix as a prefix. The
    /// function assumes that both s and prefix do not have initial padding zeros. Return an
    /// encrypted value of 1 for true and an encrypted value of 0 for false.
    pub fn starts_with_encrypted_no_init_padding(
        &self,
        s: &FheString,
        prefix: &FheString,
    ) -> RadixCiphertext {
        // First the content are compared
        let mut result = self.create_true();
        for n in 0..std::cmp::min(s.content.len(), prefix.content.len()) {
            self.integer_key.bitand_assign_parallelized(
                &mut result,
                &match prefix.padding {
                    Padding::None => self.compare_char(
                        &s.content[n],
                        &prefix.content[n],
                        std::cmp::Ordering::Equal,
                    ),
                    _ => self.integer_key.bitor_parallelized(
                        &self.compare_char(
                            &s.content[n],
                            &prefix.content[n],
                            std::cmp::Ordering::Equal,
                        ),
                        &self
                            .integer_key
                            .scalar_eq_parallelized(&prefix.content[n].0, 0),
                    ),
                },
            )
        }

        // If prefix content size is greater than s content size, check if the extra characters are
        // padding zeros
        if prefix.content.len() > s.content.len() {
            return self.integer_key.bitand_parallelized(
                &result,
                &self
                    .integer_key
                    .scalar_eq_parallelized(&prefix.content[s.content.len()].0, 0),
            );
        }
        result
    }

    /// Check if s1 encrypt the string s2, for s1 an FheString with no initial padding zeros and s2
    /// a clear &str. Return an encrypted value of 1 for true and an encrypted value of 0 for
    /// false.
    pub fn eq_clear_no_init_padding(&self, s1: &FheString, s2: &str) -> RadixCiphertext {
        if s2.len() > s1.content.len() {
            return self.create_zero();
        }
        let mut result = self.create_true();
        for n in 0..std::cmp::min(s1.content.len(), s2.len()) {
            self.integer_key.bitand_assign_parallelized(
                &mut result,
                &self.compare_clear_char(
                    &s1.content[n],
                    s2.as_bytes()[n],
                    std::cmp::Ordering::Equal,
                ),
            )
        }
        if s1.content.len() > s2.len() {
            return self.integer_key.bitand_parallelized(
                &result,
                &self
                    .integer_key
                    .scalar_eq_parallelized(&s1.content[s2.len()].0, 0),
            );
        }
        result
    }

    /// Check if s1 encrypts a string which has the clear string `prefix` as a prefix. The function
    /// assumes that both s and prefix do not have initial padding zeros. Return an encrypted value
    /// of 1 for true and an encrypted value of 0 for false.
    pub fn starts_with_clear_no_init_padding(
        &self,
        s: &FheString,
        prefix: &str,
    ) -> RadixCiphertext {
        // First the content are compared
        let mut result = self.create_true();
        for n in 0..std::cmp::min(s.content.len(), prefix.len()) {
            self.integer_key.bitand_assign_parallelized(
                &mut result,
                &self.compare_clear_char(
                    &s.content[n],
                    prefix.as_bytes()[n],
                    std::cmp::Ordering::Equal,
                ),
            )
        }
        result
    }

    /// Less or equal (<=).
    /// Check if the string encrypted by s1 is less than or equal to the string encrypted by s2.
    /// The order is the lexicographic order for bytes.
    /// Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    pub fn le(&self, s1: &FheString, s2: &FheString) -> RadixCiphertext {
        self.compare(s1, s2, std::cmp::Ordering::Less)
    }

    /// Greater or equal (>=).
    /// Check if the string encrypted by s1 is greater or equal to the string encrypted by s2.
    /// The order is the lexicographic order for bytes.
    /// Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    pub fn ge(&self, s1: &FheString, s2: &FheString) -> RadixCiphertext {
        self.compare(s1, s2, std::cmp::Ordering::Greater)
    }

    /// Less or equal (<=) clear.
    /// Check if the string encrypted by s1 is less than or equal to the clear string s2.
    /// The order is the lexicographic order for bytes.
    /// Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    pub fn le_clear(&self, s1: &FheString, s2: &str) -> RadixCiphertext {
        self.compare_clear(s1, s2, std::cmp::Ordering::Less)
    }

    /// Greater or equal (>=) clear.
    /// Check if the string encrypted by s1 is greater or equal to the clear string s2.
    /// The order is the lexicographic order for bytes.
    /// Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    pub fn ge_clear(&self, s1: &FheString, s2: &str) -> RadixCiphertext {
        self.compare_clear(s1, s2, std::cmp::Ordering::Greater)
    }

    /// Compare the encrypted strings for the lexicographic order for bytes.
    /// Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    /// If the operator is std::cmp::Ordering::Less,
    /// Return true if the string encrypted by s1 is less than or equal to the string encryptedd by
    /// s2. If the operator is std::cmp::Ordering::Greater,
    /// Return true if the string encrypted by s1 is less than or equal to the string encryptedd by
    /// s2. If the operator is std::cmp::Ordering::Equal,
    /// Return true if the string encrypted by s1 is equal to the string encryptedd by s2.
    /// For this case, using the function eq is more efficient.
    pub fn compare(
        &self,
        s1: &FheString,
        s2: &FheString,
        operator: std::cmp::Ordering,
    ) -> RadixCiphertext {
        match (s1.padding, s2.padding) {
            (Padding::None | Padding::Final, Padding::None | Padding::Final) => {
                self.compare_no_init_padding(s1, s2, operator)
            }
            (Padding::None | Padding::Final, _) => {
                self.compare_no_init_padding(s1, &self.remove_initial_padding(s2), operator)
            }
            (_, Padding::None | Padding::Final) => {
                self.compare_no_init_padding(&self.remove_initial_padding(s1), s2, operator)
            }
            _ => self.compare_no_init_padding(
                &self.remove_initial_padding(s1),
                &self.remove_initial_padding(s2),
                operator,
            ),
        }
    }

    /// Compare the encrypted string s1 with the clear string s2 for the lexicographic order for
    /// bytes. Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    /// If the operator is std::cmp::Ordering::Less,
    /// Return true if the string encrypted by s1 is less than or equal to the string s2.
    /// If the operator is std::cmp::Ordering::Greater,
    /// Return true if the string encrypted by s1 is less than or equal to the string s2.
    /// If the operator is std::cmp::Ordering::Equal,
    /// Return true if the string encrypted by s1 is equal to the string s2.
    /// For this case, using the function eq_clear is more efficient.
    pub fn compare_clear(
        &self,
        s1: &FheString,
        s2: &str,
        operator: std::cmp::Ordering,
    ) -> RadixCiphertext {
        return match s1.padding {
            Padding::None | Padding::Final => self.compare_clear_no_init_padding(s1, s2, operator),
            _ => self.compare_clear_no_init_padding(&self.remove_initial_padding(s1), s2, operator),
        };
    }

    /// Implementation of compare, for FheString without initial padding zeros.
    pub fn compare_no_init_padding(
        &self,
        s1: &FheString,
        s2: &FheString,
        operator: std::cmp::Ordering,
    ) -> RadixCiphertext {
        let mut result = self.create_zero();
        let mut equal_up_to_n_minus_1 = self.create_true();
        let mut equal_up_to_n = self.create_true();
        for n in 0..std::cmp::min(s1.content.len(), s2.content.len()) {
            equal_up_to_n = self.integer_key.bitand_parallelized(
                &equal_up_to_n_minus_1,
                &self.compare_char(&s1.content[n], &s2.content[n], std::cmp::Ordering::Equal),
            );
            result = self.integer_key.cmux_parallelized(
                &self.integer_key.bitand_parallelized(
                    &equal_up_to_n_minus_1,
                    &self.integer_key.bitnot_parallelized(&equal_up_to_n),
                ),
                &self.compare_char(&s1.content[n], &s2.content[n], operator),
                &result,
            );
            equal_up_to_n_minus_1 = equal_up_to_n.clone();
        }
        if s1.content.len() > s2.content.len() {
            return match operator {
                std::cmp::Ordering::Greater => {
                    self.integer_key.bitor_parallelized(&result, &equal_up_to_n)
                }
                _ => self.integer_key.bitor_parallelized(
                    &result,
                    &self.integer_key.bitand_parallelized(
                        &equal_up_to_n,
                        &self
                            .integer_key
                            .scalar_eq_parallelized(&s1.content[s2.content.len()].0, 0),
                    ),
                ),
            };
        }
        if s2.content.len() > s1.content.len() {
            return match operator {
                std::cmp::Ordering::Less => {
                    self.integer_key.bitor_parallelized(&result, &equal_up_to_n)
                }
                _ => self.integer_key.bitor_parallelized(
                    &result,
                    &self.integer_key.bitand_parallelized(
                        &equal_up_to_n,
                        &self
                            .integer_key
                            .scalar_eq_parallelized(&s2.content[s1.content.len()].0, 0),
                    ),
                ),
            };
        }
        self.integer_key.bitor_parallelized(&result, &equal_up_to_n)
    }

    /// Implementation of compare_clear, for FheString without initial padding zeros.
    pub fn compare_clear_no_init_padding(
        &self,
        s1: &FheString,
        s2: &str,
        operator: std::cmp::Ordering,
    ) -> RadixCiphertext {
        let mut result = self.create_zero();
        let mut equal_up_to_n_minus_1 = self.create_true();
        let mut equal_up_to_n = self.create_true();
        for n in 0..std::cmp::min(s1.content.len(), s2.len()) {
            equal_up_to_n = self.integer_key.bitand_parallelized(
                &equal_up_to_n_minus_1,
                &self.compare_clear_char(
                    &s1.content[n],
                    s2.as_bytes()[n],
                    std::cmp::Ordering::Equal,
                ),
            );
            result = self.integer_key.cmux_parallelized(
                &self.integer_key.bitand_parallelized(
                    &equal_up_to_n_minus_1,
                    &self.integer_key.bitnot_parallelized(&equal_up_to_n),
                ),
                &self.compare_clear_char(&s1.content[n], s2.as_bytes()[n], operator),
                &result,
            );
            equal_up_to_n_minus_1 = equal_up_to_n.clone();
        }
        if s1.content.len() > s2.len() {
            return match operator {
                std::cmp::Ordering::Greater => {
                    self.integer_key.bitor_parallelized(&result, &equal_up_to_n)
                }
                _ => self.integer_key.bitor_parallelized(
                    &result,
                    &self.integer_key.bitand_parallelized(
                        &equal_up_to_n,
                        &self
                            .integer_key
                            .scalar_eq_parallelized(&s1.content[s2.len()].0, 0),
                    ),
                ),
            };
        }
        if s2.len() > s1.content.len() {
            return match operator {
                std::cmp::Ordering::Less => {
                    self.integer_key.bitor_parallelized(&result, &equal_up_to_n)
                }
                _ => result,
            };
        }
        self.integer_key.bitor_parallelized(&result, &equal_up_to_n)
    }

    /// Compare the encrypted character c1 and the encrypted char c2 with the operator operator.
    /// Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    /// If the operator is std::cmp::Ordering::Less,
    /// Return true if the character encrypted by c1 is less than or equal to the character
    /// encrypted by c2. If the operator is std::cmp::Ordering::Greater,
    /// Return true if the character encrypted by c1 is greater or equal to the character encrypted
    /// by c2. If the operator is std::cmp::Ordering::Equal,
    /// Return true if the character encrypted by c1 is equal to the character encrypted by c2.
    pub fn compare_char(
        &self,
        c1: &FheAsciiChar,
        c2: &FheAsciiChar,
        operator: std::cmp::Ordering,
    ) -> RadixCiphertext {
        match operator {
            std::cmp::Ordering::Equal => self.integer_key.eq_parallelized(&c1.0, &c2.0),
            std::cmp::Ordering::Less => self.integer_key.le_parallelized(&c1.0, &c2.0),
            std::cmp::Ordering::Greater => self.integer_key.ge_parallelized(&c1.0, &c2.0),
        }
    }

    /// Compare the encrypted character c1 and the clear char c2 with the operator operator.
    /// Return an encrypted value of 1 for true and an encrypted value of 0 for false.
    /// If the operator is std::cmp::Ordering::Less,
    /// Return true if the character encrypted by c1 is less than or equal to the clear character
    /// c2. If the operator is std::cmp::Ordering::Greater,
    /// Return true if the character encrypted by c1 is greater or equal to the clear character c2.
    /// If the operator is std::cmp::Ordering::Equal,
    /// Return true if the character encrypted by c1 is equal to the clear character c2.
    pub fn compare_clear_char(
        &self,
        c: &FheAsciiChar,
        scalar: u8,
        operator: std::cmp::Ordering,
    ) -> RadixCiphertext {
        match operator {
            std::cmp::Ordering::Equal => self.integer_key.scalar_eq_parallelized(&c.0, scalar),
            std::cmp::Ordering::Less => self.integer_key.scalar_le_parallelized(&c.0, scalar),
            std::cmp::Ordering::Greater => self.integer_key.scalar_ge_parallelized(&c.0, scalar),
        }
    }

    /// Return the first element encrypting a non null character in content,
    /// replace it in content by an encryption of the null character.
    /// If all character are null, return an encryption of the null character.
    pub fn pop_first_non_zero_char(&self, content_slice: &mut [FheAsciiChar]) -> FheAsciiChar {
        let mut previous_is_padding_zero = self.create_true();
        let mut result = self.create_zero();

        for c in content_slice {
            let current_is_zero = self.integer_key.scalar_eq_parallelized(&c.0, 0);

            let first_non_null = self.integer_key.bitand_parallelized(
                &previous_is_padding_zero,
                &self.integer_key.bitnot_parallelized(&current_is_zero),
            );

            // Encrypt same value as c if c is the first no null encrypted char,
            // encrypt zero otherwise
            let to_sub = self.integer_key.mul_parallelized(&c.0, &first_non_null);

            // Compute the result
            self.integer_key
                .add_assign_parallelized(&mut result, &to_sub);

            // Update the value in content
            self.integer_key.sub_assign_parallelized(&mut c.0, &to_sub);

            // Update previous_is_padding_zero
            self.integer_key
                .bitand_assign_parallelized(&mut previous_is_padding_zero, &current_is_zero);
        }
        FheAsciiChar(result)
    }

    /// Replace the content of s with an encryption of the same string with the same
    /// and without initial padding.
    pub fn remove_initial_padding_assign(&self, s: &mut FheString) {
        let mut result_content: Vec<FheAsciiChar> = Vec::with_capacity(s.content.len());
        let mut prev_content_slice = &mut s.content.clone()[..];
        for _ in 1..s.content.len() {
            result_content.push(self.pop_first_non_zero_char(prev_content_slice));
            prev_content_slice = &mut prev_content_slice[1..];
        }
        s.padding = Padding::Final;
        s.content = result_content;
    }

    /// Return an encryption of the same string, with the same content length,
    /// without initial padding.
    pub fn remove_initial_padding(&self, s: &FheString) -> FheString {
        let mut result_content: Vec<FheAsciiChar> = Vec::with_capacity(s.content.len());
        let mut prev_content_slice = &mut s.content.clone()[..];
        for _ in 0..s.content.len() {
            result_content.push(self.pop_first_non_zero_char(prev_content_slice));
            prev_content_slice = &mut prev_content_slice[1..];
        }
        FheString {
            content: result_content,
            padding: Padding::Final,
            length: s.length.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ciphertext::{
        decrypt_fhe_string, encrypt_ascii_vec, gen_keys, FheStrLength, Padding,
    };
    use crate::server_key::StringServerKey;
    use lazy_static::lazy_static;
    use tfhe::integer::RadixClientKey;

    lazy_static! {
        pub static ref KEYS: (RadixClientKey, StringServerKey) = gen_keys();
    }

    #[test]
    fn test_pop_first_non_zero_char() {
        let mut encrypted_str = encrypt_ascii_vec(
            &KEYS.0,
            &vec![0, 97, 98, 0],
            Padding::InitialAndFinal,
            FheStrLength::Clear(1),
        )
        .unwrap();
        let poped_char = KEYS
            .1
            .pop_first_non_zero_char(&mut encrypted_str.content[..]);
        let decrypted_poped_char = KEYS.0.decrypt::<u8>(&poped_char.0);
        assert_eq!(decrypted_poped_char, 97);
        let decrypted_string = decrypt_fhe_string(&KEYS.0, &encrypted_str).unwrap();
        assert_eq!(decrypted_string, "b");
    }

    #[test]
    fn test_remove_initial_padding_assign() {
        let mut encrypted_str = encrypt_ascii_vec(
            &KEYS.0,
            &vec![0, 97],
            Padding::InitialAndFinal,
            FheStrLength::Clear(1),
        )
        .unwrap();
        KEYS.1.remove_initial_padding_assign(&mut encrypted_str);
        let decrypted_char = KEYS.0.decrypt::<u8>(&encrypted_str.content[0].0);
        assert_eq!(decrypted_char, 97);
        assert_eq!(encrypted_str.padding, Padding::Final);

        let decrypted_string = decrypt_fhe_string(&KEYS.0, &encrypted_str).unwrap();
        assert_eq!(decrypted_string, "a");
    }

    #[test]
    fn test_remove_initial_padding() {
        let encrypted_str = encrypt_ascii_vec(
            &KEYS.0,
            &vec![0, 97],
            Padding::InitialAndFinal,
            FheStrLength::Clear(1),
        )
        .unwrap();
        let encrypted_str_no_padding = KEYS.1.remove_initial_padding(&encrypted_str);
        let decrypted_char = KEYS.0.decrypt::<u8>(&encrypted_str_no_padding.content[0].0);
        assert_eq!(decrypted_char, 97);
        assert_eq!(encrypted_str_no_padding.padding, Padding::Final);

        let decrypted_string = decrypt_fhe_string(&KEYS.0, &encrypted_str_no_padding).unwrap();
        assert_eq!(decrypted_string, "a");
    }

    #[test]
    fn test_eq() {
        let encrypted_str1 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![97, 0],
            Padding::InitialAndFinal,
            FheStrLength::Clear(1),
        )
        .unwrap();
        let encrypted_str2 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![98],
            Padding::InitialAndFinal,
            FheStrLength::Clear(1),
        )
        .unwrap();
        let eq_str1_str2 = KEYS.1.eq(&encrypted_str1, &encrypted_str2);
        let clear_eq_str1_str2 = KEYS.0.decrypt::<u8>(&eq_str1_str2);
        assert_eq!(clear_eq_str1_str2, 0);
    }

    #[test]
    fn test_le_ge() {
        let encrypted_str1 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![99, 100, 101],
            Padding::InitialAndFinal,
            FheStrLength::Clear(1),
        )
        .unwrap();
        let encrypted_str2 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![99, 101],
            Padding::InitialAndFinal,
            FheStrLength::Clear(1),
        )
        .unwrap();

        let le_str1_str2 = KEYS.1.le(&encrypted_str1, &encrypted_str2);
        let ge_str1_str2 = KEYS.1.ge(&encrypted_str1, &encrypted_str2);

        let clear_le_str1_str2 = KEYS.0.decrypt::<u8>(&le_str1_str2);
        let clear_ge_str1_str2 = KEYS.0.decrypt::<u8>(&ge_str1_str2);

        assert_eq!(clear_le_str1_str2, 1);
        assert_eq!(clear_ge_str1_str2, 0);
    }

    #[test]
    fn test_eq() {
        let encrypted_str1 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![98, 0],
            Padding::InitialAndFinal,
            FheStrLength::Clear(2),
        )
        .unwrap();
        let encrypted_str2 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![0, 98, 99],
            Padding::InitialAndFinal,
            FheStrLength::Clear(2),
        )
        .unwrap();

        let eq_str1_str2 = KEYS.1.eq(&encrypted_str1, &encrypted_str2);
        let clear_eq_str1_str2 = KEYS.0.decrypt::<u8>(&eq_str1_str2);

        assert_eq!(clear_eq_str1_str2, 0);
    }

    #[test]
    fn test_neq() {
        let encrypted_str1 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![98, 97],
            Padding::InitialAndFinal,
            FheStrLength::Clear(2),
        )
        .unwrap();
        let encrypted_str2 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![98],
            Padding::InitialAndFinal,
            FheStrLength::Clear(2),
        )
        .unwrap();

        let eq_str1_str2 = KEYS.1.eq(&encrypted_str1, &encrypted_str2);
        let clear_eq_str1_str2 = KEYS.0.decrypt::<u8>(&eq_str1_str2);

        assert_eq!(clear_eq_str1_str2, 0);
    }

    #[test]
    fn test_le_ge_clear() {
        let encrypted_str1 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![98, 100, 0],
            Padding::Final,
            FheStrLength::Clear(2),
        )
        .unwrap();

        let le_str1_str2 = KEYS.1.le_clear(&encrypted_str1, "bd");
        let ge_str1_str2 = KEYS.1.ge_clear(&encrypted_str1, "ada");

        let clear_le_str1_str2 = KEYS.0.decrypt::<u8>(&le_str1_str2);
        let clear_ge_str1_str2 = KEYS.0.decrypt::<u8>(&ge_str1_str2);

        assert_eq!(clear_le_str1_str2, 1);
        assert_eq!(clear_ge_str1_str2, 1);
    }

    #[test]
    fn test_eq_clear() {
        let encrypted_str1 = encrypt_ascii_vec(
            &KEYS.0,
            &vec![0, 0],
            Padding::InitialAndFinal,
            FheStrLength::Encrypted(KEYS.1.create_zero()),
        )
        .unwrap();

        let eq_str1_str2 = KEYS.1.eq_clear(&encrypted_str1, "");
        let eq_str1_str3 = KEYS.1.eq_clear(&encrypted_str1, "b");
        let eq_str1_str4 = KEYS.1.eq_clear(&encrypted_str1, "bd");

        let clear_eq_str1_str2 = KEYS.0.decrypt::<u8>(&eq_str1_str2);
        let clear_eq_str1_str3 = KEYS.0.decrypt::<u8>(&eq_str1_str3);
        let clear_eq_str1_str4 = KEYS.0.decrypt::<u8>(&eq_str1_str4);

        assert_eq!(clear_eq_str1_str2, 1);
        assert_eq!(clear_eq_str1_str3, 0);
        assert_eq!(clear_eq_str1_str4, 0);
    }

    #[test]
    fn test_starts_with_encrypted() {
        let encrypted_str = encrypt_ascii_vec(
            &KEYS.0,
            &vec![0, 98, 99],
            Padding::InitialAndFinal,
            FheStrLength::Clear(2),
        )
        .unwrap();
        let encrypted_prefix = encrypt_ascii_vec(
            &KEYS.0,
            &vec![98],
            Padding::InitialAndFinal,
            FheStrLength::Clear(2),
        )
        .unwrap();

        let starts_with_result = KEYS
            .1
            .starts_with_encrypted(&encrypted_str, &encrypted_prefix);
        let clear_result = KEYS.0.decrypt::<u8>(&starts_with_result);

        assert_eq!(clear_result, 1);
    }

    #[test]
    fn test_starts_with_clear() {
        let encrypted_str = encrypt_ascii_vec(
            &KEYS.0,
            &vec![98, 99],
            Padding::InitialAndFinal,
            FheStrLength::Clear(2),
        )
        .unwrap();

        let mut starts_with_result = KEYS.1.starts_with_clear(&encrypted_str, "b");
        let clear_result = KEYS.0.decrypt::<u8>(&starts_with_result);
        assert_eq!(clear_result, 1);

        starts_with_result = KEYS.1.starts_with_clear(&encrypted_str, "");
        let clear_result = KEYS.0.decrypt::<u8>(&starts_with_result);
        assert_eq!(clear_result, 1);

        starts_with_result = KEYS.1.starts_with_clear(&encrypted_str, "bc");
        let clear_result = KEYS.0.decrypt::<u8>(&starts_with_result);
        assert_eq!(clear_result, 1);

        starts_with_result = KEYS.1.starts_with_clear(&encrypted_str, "def");
        let clear_result = KEYS.0.decrypt::<u8>(&starts_with_result);
        assert_eq!(clear_result, 0);

        starts_with_result = KEYS.1.starts_with_clear(&encrypted_str, "d");
        let clear_result = KEYS.0.decrypt::<u8>(&starts_with_result);
        assert_eq!(clear_result, 0);
    }
}
