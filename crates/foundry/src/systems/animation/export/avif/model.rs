use std::slice;

use super::tables::{
    ALL_ZERO_CDF, ANGLE_DELTA_CDF, BASE_CDF, BASE_EOB_CDF, DC_SIGN_CDF, DELTA_Q_CDF, DRL_MODE_CDF, EOB_16_CDF, EOB_64_CDF, EOB_256_CDF, EOB_1024_CDF, EOB_EXTRA_CDF,
    INTER_TX_ONE_CDF, INTER_TX_TWO_CDF, INTER_TX_TYPE_CDF, INTER_Y_MODE_CDF, IS_INTER_CDF, MV_BIT_CDF, MV_CLASS_CDF, MV_CLASS0_BIT_CDF, MV_CLASS0_FR_CDF, MV_FR_CDF, MV_JOINT_CDF, MV_SIGN_CDF,
    NEW_MV_CDF, PARTITION_8_CDF, PARTITION_16_CDF, PARTITION_32_CDF, PARTITION_64_CDF, RANGE_CDF, REF_MV_CDF, SINGLE_REF_CDF, SKIP_CDF, TX_TYPE_NARROW_CDF,
    TX_TYPE_WIDE_CDF, UV_MODE_LUMA_CDF, UV_MODE_PLAIN_CDF, Y_MODE_CDF, ZERO_MV_CDF,
};

#[derive(Clone)]
pub(super) struct Model {
    pub partition_8: [[u16; 5]; 4],
    pub partition_16: [[u16; 11]; 4],
    pub partition_32: [[u16; 11]; 4],
    pub partition_64: [[u16; 11]; 4],
    pub skip: [[u16; 3]; 3],
    pub y_mode: [[[u16; 14]; 5]; 5],
    pub uv_mode_plain: [[u16; 14]; 13],
    pub uv_mode_luma: [[u16; 15]; 13],
    pub angle_delta: [[u16; 8]; 8],
    pub tx_type_wide: [[[u16; 8]; 13]; 2],
    pub tx_type_narrow: [[[u16; 6]; 13]; 3],
    pub all_zero: [[[u16; 3]; 13]; 5],
    pub eob_16: [[[u16; 6]; 2]; 2],
    pub eob_64: [[[u16; 8]; 2]; 2],
    pub eob_256: [[[u16; 10]; 2]; 2],
    pub eob_1024: [[u16; 12]; 2],
    pub eob_extra: [[[[u16; 3]; 9]; 2]; 5],
    pub dc_sign: [[[u16; 3]; 3]; 2],
    pub base_eob: [[[[u16; 4]; 4]; 2]; 5],
    pub base: [[[[u16; 5]; 42]; 2]; 5],
    pub range: [[[[u16; 5]; 21]; 2]; 5],
    pub inter_y_mode: [[u16; 14]; 4],
    pub is_inter: [[u16; 3]; 4],
    pub single_ref: [[[u16; 3]; 6]; 3],
    pub new_mv: [[u16; 3]; 6],
    pub zero_mv: [[u16; 3]; 2],
    pub ref_mv: [[u16; 3]; 6],
    pub drl_mode: [[u16; 3]; 3],
    pub mv_joint: [u16; 5],
    pub mv_class: [[u16; 12]; 2],
    pub mv_class0_bit: [[u16; 3]; 2],
    pub mv_class0_fr: [[[u16; 5]; 2]; 2],
    pub mv_fr: [[u16; 5]; 2],
    pub mv_sign: [[u16; 3]; 2],
    pub mv_bit: [[[u16; 3]; 10]; 2],
    pub inter_tx_type: [[u16; 3]; 4],
    pub inter_tx_one: [[u16; 17]; 2],
    pub inter_tx_two: [u16; 13],
    pub delta_q: [u16; 5],
}

impl Model {
    pub(super) fn new(base_q: u8) -> Self {
        let band = match base_q {
            0..=20 => 0,
            21..=60 => 1,
            61..=120 => 2,
            _ => 3,
        };
        Self {
            partition_8: PARTITION_8_CDF,
            partition_16: PARTITION_16_CDF,
            partition_32: PARTITION_32_CDF,
            partition_64: PARTITION_64_CDF,
            skip: SKIP_CDF,
            y_mode: Y_MODE_CDF,
            uv_mode_plain: UV_MODE_PLAIN_CDF,
            uv_mode_luma: UV_MODE_LUMA_CDF,
            angle_delta: ANGLE_DELTA_CDF,
            tx_type_wide: TX_TYPE_WIDE_CDF,
            tx_type_narrow: TX_TYPE_NARROW_CDF,
            all_zero: ALL_ZERO_CDF[band],
            eob_16: EOB_16_CDF[band],
            eob_64: EOB_64_CDF[band],
            eob_256: EOB_256_CDF[band],
            eob_1024: EOB_1024_CDF[band],
            eob_extra: EOB_EXTRA_CDF[band],
            dc_sign: DC_SIGN_CDF[band],
            base_eob: BASE_EOB_CDF[band],
            base: BASE_CDF[band],
            range: RANGE_CDF[band],
            inter_y_mode: INTER_Y_MODE_CDF,
            is_inter: IS_INTER_CDF,
            single_ref: SINGLE_REF_CDF,
            new_mv: NEW_MV_CDF,
            zero_mv: ZERO_MV_CDF,
            ref_mv: REF_MV_CDF,
            drl_mode: DRL_MODE_CDF,
            mv_joint: MV_JOINT_CDF,
            mv_class: MV_CLASS_CDF,
            mv_class0_bit: [MV_CLASS0_BIT_CDF; 2],
            mv_class0_fr: MV_CLASS0_FR_CDF,
            mv_fr: MV_FR_CDF,
            mv_sign: [MV_SIGN_CDF; 2],
            mv_bit: [MV_BIT_CDF; 2],
            inter_tx_type: INTER_TX_TYPE_CDF,
            inter_tx_one: INTER_TX_ONE_CDF,
            inter_tx_two: INTER_TX_TWO_CDF,
            delta_q: DELTA_Q_CDF,
        }
    }

    pub(super) fn carried(&self) -> Self {
        let mut model = self.clone();
        model.restart();
        model
    }

    fn restart(&mut self) {
        fresh(&mut self.partition_8);
        fresh(&mut self.partition_16);
        fresh(&mut self.partition_32);
        fresh(&mut self.partition_64);
        fresh(&mut self.skip);
        fresh(self.y_mode.as_flattened_mut());
        fresh(&mut self.uv_mode_plain);
        fresh(&mut self.uv_mode_luma);
        fresh(&mut self.angle_delta);
        fresh(self.tx_type_wide.as_flattened_mut());
        fresh(self.tx_type_narrow.as_flattened_mut());
        fresh(self.all_zero.as_flattened_mut());
        fresh(self.eob_16.as_flattened_mut());
        fresh(self.eob_64.as_flattened_mut());
        fresh(self.eob_256.as_flattened_mut());
        fresh(&mut self.eob_1024);
        fresh(self.eob_extra.as_flattened_mut().as_flattened_mut());
        fresh(self.dc_sign.as_flattened_mut());
        fresh(self.base_eob.as_flattened_mut().as_flattened_mut());
        fresh(self.base.as_flattened_mut().as_flattened_mut());
        fresh(self.range.as_flattened_mut().as_flattened_mut());
        fresh(&mut self.inter_y_mode);
        fresh(&mut self.is_inter);
        fresh(self.single_ref.as_flattened_mut());
        fresh(&mut self.new_mv);
        fresh(&mut self.zero_mv);
        fresh(&mut self.ref_mv);
        fresh(&mut self.drl_mode);
        fresh(slice::from_mut(&mut self.mv_joint));
        fresh(&mut self.mv_class);
        fresh(&mut self.mv_class0_bit);
        fresh(self.mv_class0_fr.as_flattened_mut());
        fresh(&mut self.mv_fr);
        fresh(&mut self.mv_sign);
        fresh(self.mv_bit.as_flattened_mut());
        fresh(&mut self.inter_tx_type);
        fresh(&mut self.inter_tx_one);
        fresh(slice::from_mut(&mut self.inter_tx_two));
        fresh(slice::from_mut(&mut self.delta_q));
    }
}

fn fresh<const N: usize>(cdfs: &mut [[u16; N]]) {
    for cdf in cdfs {
        cdf[N - 1] = 0;
    }
}
